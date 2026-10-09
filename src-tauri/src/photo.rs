//! A photo, as the backend knows it: one identity (its path) and everything that follows from it.
//!
//! The commands that concern a single photo used to be free functions taking a path string, each
//! working out for itself where the photo lives and which of its files to touch: is it a file, an
//! Apple Photos asset, an Immich one; where is its sidecar, its developed `.preview.jpg`, the JPEG
//! the camera wrote beside it. That knowledge is gathered here, once, so a command reads
//! `Photo::new(path).set_rating(5)` instead of re-deriving it.
//!
//! It is the Rust counterpart of the interface's `Photo` model (modules/core/models/Photo.js): that
//! one says what a photo looks like; this one knows where it lives and what can be done to it.
//! What has to be *written* to its files goes through [`crate::photo_writes`] when it should wait
//! for the person to stop (clearing the develop settings), and is done directly when it should not
//! (a rating, today).
//!
//! A `Photo` holds no file handle and reads nothing until asked: it is a path with manners, cheap to
//! make and to clone.

use std::path::{Path, PathBuf};

use crate::{apple_photos, immich, preview};

/// Where a photo's pixels come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    /// A file on disk: the archive, a card, a NAS.
    File,
    /// An asset of the Photos library on this Mac (`apple-photos://…`): read-only.
    ApplePhotos,
    /// An asset on an Immich server (`immich://…`): remote, read-only.
    Immich,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Photo {
    path: String,
}

impl Photo {
    pub(crate) fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    /// The identifier everything else keys on: the file's path, or the asset's URL.
    #[allow(dead_code)] // part of the model's surface; callers adopt it as they move over
    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) fn origin(&self) -> Origin {
        if apple_photos::is_asset(&self.path) {
            Origin::ApplePhotos
        } else if immich::is_asset(&self.path) {
            Origin::Immich
        } else {
            Origin::File
        }
    }

    /// A real file, whose folder can be written to (as opposed to a library asset).
    pub(crate) fn is_file(&self) -> bool {
        self.origin() == Origin::File
    }

    /// The last component: `IMG_0001.RAF`.
    #[allow(dead_code)] // part of the model's surface; callers adopt it as they move over
    pub(crate) fn file_name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }

    /// The name without its extension: `IMG_0001`. What a story note and a sidecar are keyed on.
    #[allow(dead_code)] // part of the model's surface; callers adopt it as they move over
    pub(crate) fn stem(&self) -> &str {
        let name = self.file_name();
        match name.rfind('.') {
            Some(dot) if dot > 0 => &name[..dot],
            _ => name,
        }
    }

    /// The photo as a filesystem path. Only meaningful for [`Origin::File`].
    pub(crate) fn as_path(&self) -> &Path {
        Path::new(&self.path)
    }

    /// The developed `.preview.jpg` beside the original (or in the Photos edits folder): the
    /// durable truth about how the photo looks once developed.
    pub(crate) fn preview_sidecar(&self) -> Option<PathBuf> {
        preview::preview_sidecar_path(self.as_path())
    }

    /// A real JPEG the camera wrote next to the RAW (RAW+JPEG shooting), if there is one.
    pub(crate) fn companion_jpeg(&self) -> Option<PathBuf> {
        preview::companion_jpeg_path(self.as_path())
    }

    /// Where the photo's metadata sidecar lives (for a file, next to it).
    pub(crate) fn metadata_path(&self) -> Result<PathBuf, String> {
        apple_photos::metadata_path(&self.path)
    }

    /// Change the photo's metadata sidecar (rating, caption, engine…), serialised with every other
    /// such change.
    pub(crate) fn update_metadata(
        &self,
        update: impl FnOnce(&mut reveal_meta::Sidecar) -> Result<(), String>,
    ) -> Result<(), String> {
        apple_photos::update_metadata(&self.path, update)
    }

    /// The photo's metadata sidecar as it is on disk, or `None` if it has none yet.
    ///
    /// What the photo is still owed (an edit waiting out its quiet period) is written first, so a
    /// read never sees the sidecar as it was a moment ago. Every read of a photo's recipe goes
    /// through here for that reason — export, publishing, opening the photo.
    pub(crate) fn sidecar(&self) -> Result<Option<reveal_meta::Sidecar>, String> {
        crate::photo_writes::flush_photo(&self.path);
        let metadata = self.metadata_path()?;
        reveal_meta::read(&metadata).map_err(|e| e.to_string())
    }

    /// The recipe a sidecar holds: the engine's settings as they were saved, or the default recipe
    /// when it holds none or they cannot be read (a photo never developed develops as default).
    pub(crate) fn recipe_in(sidecar: Option<&reveal_meta::Sidecar>) -> reveal_engine::Recipe {
        sidecar
            .and_then(|s| s.engine_settings.clone())
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default()
    }

    /// The photo's recipe, read the way every export and publication reads it (see [`Self::sidecar`]:
    /// what is still owed to the photo is written first).
    pub(crate) fn recipe(&self) -> Result<reveal_engine::Recipe, String> {
        Ok(Self::recipe_in(self.sidecar()?.as_ref()))
    }

    /// The photo's caption, if it has one.
    pub(crate) fn caption(&self) -> Result<Option<String>, String> {
        Ok(self.sidecar()?.and_then(|s| s.description))
    }

    /// Star rating, 0–5 (above that is clamped). Written at once: a rating is cheap and the person
    /// is looking at it.
    pub(crate) fn set_rating(&self, rating: u8) -> Result<(), String> {
        self.update_metadata(|sidecar| {
            sidecar.rating = Some(rating.min(5));
            Ok(())
        })
    }

    /// Capture timestamp in seconds since Unix epoch. Stored in sidecar so it survives
    /// fresh rescans and index rebuilds.
    pub(crate) fn set_capture_at(&self, timestamp: i64) -> Result<(), String> {
        self.update_metadata(|sidecar| {
            sidecar.capture_at = Some(timestamp);
            Ok(())
        })
    }

    /// Write the recipe into the photo's sidecar: the engine, and the engine's settings as they
    /// are. Done at once; edits that can wait go through [`crate::photo_writes`].
    pub(crate) fn save_recipe(&self, recipe: &reveal_engine::Recipe) -> Result<(), String> {
        self.update_metadata(|sidecar| {
            sidecar.engine = Some(recipe.engine.clone());
            sidecar.engine_settings = Some(serde_json::to_value(recipe).map_err(|e| e.to_string())?);
            Ok(())
        })
    }

    /// Take the develop settings off the photo now: the engine and its settings out of the sidecar,
    /// and the developed `.preview.jpg` deleted, so the grid and the viewer fall back to the camera's
    /// own picture. Callers that can wait go through [`crate::photo_writes`] instead.
    pub(crate) fn clear_development_now(&self) -> Result<(), String> {
        self.update_metadata(|sidecar| {
            sidecar.engine = None;
            sidecar.engine_settings = None;
            Ok(())
        })?;
        // The Swift-era `.reveal.jpg` is left untouched — a read-only fallback.
        if let Some(candidate) = self.preview_sidecar() {
            match std::fs::remove_file(&candidate) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_knows_where_a_photo_comes_from() {
        assert_eq!(Photo::new("/Volumes/nas/IMG_0001.RAF").origin(), Origin::File);
        assert_eq!(Photo::new("apple-photos://6162/IMG_0001.HEIC").origin(), Origin::ApplePhotos);
        assert_eq!(Photo::new("immich://abc/IMG_0001.JPG").origin(), Origin::Immich);
        assert!(Photo::new("/a/b.RAF").is_file());
        assert!(!Photo::new("immich://abc/IMG_0001.JPG").is_file());
    }

    #[test]
    fn it_names_a_photo_the_way_stories_and_sidecars_do() {
        let p = Photo::new("/Volumes/nas/2026/IMG_0001.RAF");
        assert_eq!(p.file_name(), "IMG_0001.RAF");
        assert_eq!(p.stem(), "IMG_0001");
        // Only the last extension goes, and a bare name is its own stem.
        assert_eq!(Photo::new("/a/IMG_0001.preview.jpg").stem(), "IMG_0001.preview");
        assert_eq!(Photo::new("/a/README").stem(), "README");
        assert_eq!(Photo::new("/a/.hidden").stem(), ".hidden");
        assert_eq!(Photo::new("IMG.RAF").file_name(), "IMG.RAF");
    }

    #[test]
    fn the_developed_preview_sits_beside_the_original() {
        let p = Photo::new("/Volumes/nas/DSCF3098.RAF");
        assert_eq!(p.preview_sidecar(), Some(PathBuf::from("/Volumes/nas/DSCF3098.preview.jpg")));
    }

    #[test]
    fn the_companion_jpeg_is_found_only_when_it_exists() {
        let dir = tempfile::tempdir().unwrap();
        let raf = dir.path().join("DSCF0001.RAF");
        std::fs::write(&raf, b"raw").unwrap();
        let photo = Photo::new(raf.to_string_lossy());
        assert_eq!(photo.companion_jpeg(), None);
        let jpg = dir.path().join("DSCF0001.JPG");
        std::fs::write(&jpg, b"jpeg").unwrap();
        assert_eq!(photo.companion_jpeg(), Some(jpg));
    }

    fn photo_in(dir: &tempfile::TempDir, name: &str) -> Photo {
        let path = dir.path().join(name);
        std::fs::write(&path, b"raw").unwrap();
        Photo::new(path.to_string_lossy())
    }

    #[test]
    fn a_rating_is_written_to_the_sidecar_and_clamped_to_five() {
        let dir = tempfile::tempdir().unwrap();
        let photo = photo_in(&dir, "IMG_0001.RAF");
        assert_eq!(photo.sidecar().unwrap(), None);
        photo.set_rating(4).unwrap();
        assert_eq!(photo.sidecar().unwrap().unwrap().rating, Some(4));
        photo.set_rating(9).unwrap();
        assert_eq!(photo.sidecar().unwrap().unwrap().rating, Some(5));
        // No stars is stored as no rating at all (the sidecar leaves the field out).
        photo.set_rating(0).unwrap();
        assert_eq!(photo.sidecar().unwrap().unwrap().rating, None);
    }

    #[test]
    fn a_saved_recipe_is_what_the_sidecar_holds_and_a_second_one_replaces_it() {
        let dir = tempfile::tempdir().unwrap();
        let photo = photo_in(&dir, "IMG_0003.RAF");
        let mut recipe = reveal_engine::Recipe { engine: "rapid".to_string(), ..Default::default() };
        photo.save_recipe(&recipe).unwrap();
        let sidecar = photo.sidecar().unwrap().unwrap();
        assert_eq!(sidecar.engine.as_deref(), Some("rapid"));
        assert!(sidecar.engine_settings.is_some());
        recipe.engine = "spektra".to_string();
        photo.save_recipe(&recipe).unwrap();
        assert_eq!(photo.sidecar().unwrap().unwrap().engine.as_deref(), Some("spektra"));
    }

    #[test]
    fn clearing_the_develop_settings_keeps_the_rating_and_removes_the_developed_preview() {
        let dir = tempfile::tempdir().unwrap();
        let photo = photo_in(&dir, "IMG_0002.RAF");
        photo.set_rating(3).unwrap();
        photo
            .update_metadata(|s| {
                s.engine = Some("rapid".into());
                Ok(())
            })
            .unwrap();
        let preview = photo.preview_sidecar().unwrap();
        std::fs::write(&preview, b"developed").unwrap();

        photo.clear_development_now().unwrap();

        let sidecar = photo.sidecar().unwrap().unwrap();
        assert_eq!(sidecar.engine, None);
        assert_eq!(sidecar.rating, Some(3), "what is not develop settings is left alone");
        assert!(!preview.exists(), "the developed preview is gone");
        // And clearing again, with nothing left to clear, is not an error.
        photo.clear_development_now().unwrap();
    }

    #[test]
    fn a_photo_never_developed_has_the_default_recipe_and_no_caption() {
        let dir = tempfile::tempdir().unwrap();
        let photo = photo_in(&dir, "IMG_0005.RAF");
        assert_eq!(photo.recipe().unwrap().engine, reveal_engine::Recipe::default().engine);
        assert_eq!(photo.caption().unwrap(), None);
    }

    #[test]
    fn the_recipe_and_the_caption_are_what_was_saved() {
        let dir = tempfile::tempdir().unwrap();
        let photo = photo_in(&dir, "IMG_0006.RAF");
        let recipe = reveal_engine::Recipe { engine: "rapid".to_string(), film_prep: 0.6, ..Default::default() };
        photo.save_recipe(&recipe).unwrap();
        photo
            .update_metadata(|s| {
                s.description = Some("the ceremony".to_string());
                Ok(())
            })
            .unwrap();
        let read = photo.recipe().unwrap();
        assert_eq!(read.engine, "rapid");
        assert!((read.film_prep - 0.6).abs() < 1e-6, "the settings come back whole");
        assert_eq!(photo.caption().unwrap().as_deref(), Some("the ceremony"));
    }

    #[test]
    fn settings_that_cannot_be_read_fall_back_to_the_default_recipe() {
        let broken = reveal_meta::Sidecar {
            engine_settings: Some(serde_json::json!({ "exposure_ev": "not a number" })),
            ..Default::default()
        };
        assert_eq!(Photo::recipe_in(Some(&broken)).engine, reveal_engine::Recipe::default().engine);
        assert_eq!(Photo::recipe_in(None).engine, reveal_engine::Recipe::default().engine);
    }

    /// A disk that really writes the sidecar, to watch the queue and the reads meet.
    struct SidecarDisk;
    impl crate::photo_writes::Disk for SidecarDisk {
        fn save_recipe(&self, path: &str, recipe: &reveal_engine::Recipe) -> Result<(), String> {
            Photo::new(path).save_recipe(recipe)
        }
        fn clear_development(&self, path: &str) -> Result<(), String> {
            Photo::new(path).clear_development_now()
        }
        fn set_rating(&self, path: &str, stars: u8) -> Result<(), String> {
            Photo::new(path).set_rating(stars)
        }
    }

    #[test]
    fn reading_a_photos_sidecar_first_pays_what_the_photo_is_still_owed() {
        let dir = tempfile::tempdir().unwrap();
        let photo = photo_in(&dir, "IMG_0004.RAF");
        // The app's queue, with a quiet period far longer than the test: only a read can pay it.
        let writes = crate::photo_writes::PhotoWrites::start(SidecarDisk, std::time::Duration::from_secs(600));
        crate::photo_writes::install(writes.clone());

        let recipe = reveal_engine::Recipe { engine: "spektra".to_string(), ..Default::default() };
        writes.queue_save_recipe(photo.path(), recipe);
        assert_eq!(writes.pending(), 1, "the edit is waiting out its quiet period");

        // Export, publishing and opening the photo all read through here, and must see the edit.
        let seen = photo.sidecar().unwrap().expect("the edit reached the sidecar");
        assert_eq!(seen.engine.as_deref(), Some("spektra"));
        assert_eq!(writes.pending(), 0);
        writes.close();
    }
}
