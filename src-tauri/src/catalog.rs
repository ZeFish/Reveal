//! The library on disk: picking and listing folders, moving and renaming, ratings, catalogue roots, and the SQLite index they feed.
//!
//! Lifted out of `lib.rs` unchanged — see that file's header.

use crate::*;

/// Native folder picker for the cull grid.
#[tauri::command]
pub(crate) async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .blocking_pick_folder()
        .and_then(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

/// True iff `path` is an existing directory we can write into. Used by the
/// import reachability guard to catch a stale NAS mount before copying. We
/// probe by creating a temp file (the only reliable cross-FS writability
/// test on macOS — `metadata().permissions().readonly()` is unreliable on
/// network volumes and ignores ACLs).
pub(crate) fn is_writable_dir(path: &std::path::Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let probe = path.join(format!(".reveal-write-probe-{stamp}"));
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[derive(serde::Serialize)]
pub(crate) struct FrameInfo {
    path: String,
    name: String,
    rating: u8,
}

/// The RAW frames of one folder (non-recursive), with sidecar ratings.
#[tauri::command]
pub(crate) async fn list_dir(path: String) -> Result<Vec<FrameInfo>, String> {
    crate::blocking(move || list_dir_blocking(&path)).await
}

fn list_dir_blocking(path: &str) -> Result<Vec<FrameInfo>, String> {
    let mut frames = Vec::new();
    for e in std::fs::read_dir(path).map_err(|e| e.to_string())? {
        let Ok(e) = e else { continue };
        let p = e.path();
        // macOS writes a "._name.raf" AppleDouble sidecar next to every real
        // file on NFS/SMB volumes — same extension, not a photo, unreadable as
        // one (permanent decode failures). Skip it, like the indexer does.
        if e.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let ext = p
            .extension()
            .and_then(|x| x.to_str())
            .map(str::to_lowercase)
            .unwrap_or_default();
        if !reveal_decode::RAW_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }
        let rating = crate::photo::Photo::new(p.to_string_lossy())
            .sidecar()
            .ok()
            .flatten()
            .and_then(|s| s.rating)
            .unwrap_or(0);
        frames.push(FrameInfo {
            name: p.file_name().unwrap_or_default().to_string_lossy().into_owned(),
            path: p.to_string_lossy().into_owned(),
            rating,
        });
    }
    frames.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(frames)
}

/// Star rating 0-5 — sidecar is the truth, the index mirrors it.
///
/// The index (local, cheap) is updated at once, so filters and sorting follow the stars the moment
/// they are pressed. The sidecar of a file lives on the NAS: the write is owed to the photo and
/// paid once it has been left alone (see `photo_writes`) — pressing 1, 2, 3 on the way to 4 is one
/// write. A library asset's sidecar is in the app's own folder, so it is written straight away.
#[tauri::command]
pub(crate) async fn set_rating(
    index: tauri::State<'_, IndexState>,
    writes: tauri::State<'_, crate::photo_writes::PhotoWritesState>,
    path: String,
    rating: u8,
) -> Result<(), String> {
    let photo = crate::photo::Photo::new(path.clone());
    let rating = rating.min(5);
    if !photo.is_file() {
        let for_sidecar = photo.clone();
        return crate::blocking(move || for_sidecar.set_rating(rating)).await;
    }
    writes.0.queue_set_rating(&path, rating);
    index.0.set_rating(&path, rating).map_err(|e| e.to_string())
}

/// Index (or re-index) the archive root. Synchronous — returns the stats.
#[tauri::command]
pub(crate) async fn scan_root(
    app: tauri::AppHandle,
    index: tauri::State<'_, IndexState>,
    path: String,
) -> Result<reveal_index::ScanStats, String> {
    let idx = index.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        // Live progress for the sidebar's library row (the Swift spinner +
        // count) — throttled so a fast local walk doesn't flood the webview.
        let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
        let stats = idx.scan_with(std::path::Path::new(&path), |dirs, frames| {
            if last.elapsed().as_millis() >= 400 {
                last = std::time::Instant::now();
                let _ = app.emit(
                    "index-progress",
                    serde_json::json!({ "dirs": dirs, "frames": frames }),
                );
            }
        });
        let _ = app.emit("index-progress", serde_json::json!({ "done": true }));
        if let Ok(s) = &stats {
            eprintln!(
                "scan {path}: {} frames ({} new, {} removed), {} folders, {} ms",
                s.frames, s.added, s.removed, s.dirs, s.ms
            );
        }
        stats.map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Reconcile one indexed folder without changing the catalogue root.
#[tauri::command]
pub(crate) async fn scan_folder(
    app: tauri::AppHandle,
    index: tauri::State<'_, IndexState>,
    path: String,
) -> Result<reveal_index::ScanStats, String> {
    // Valid if the folder sits under ANY registered root (multi-root), not
    // just the single legacy pointer.
    let containing = index
        .0
        .library_covering(&path)
        .map_err(|e| e.to_string())?;
    if containing.is_none() {
        return Err("Folder is outside every indexed catalogue".to_string());
    }
    let folder = std::path::PathBuf::from(&path);
    let idx = index.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
        let stats = idx.scan_subtree_with(&folder, |dirs, frames| {
            if last.elapsed().as_millis() >= 400 {
                last = std::time::Instant::now();
                let _ = app.emit(
                    "index-progress",
                    serde_json::json!({ "dirs": dirs, "frames": frames }),
                );
            }
        });
        let _ = app.emit("index-progress", serde_json::json!({ "done": true }));
        stats.map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Move a photo into `dest_dir` — the RAW plus its `.xmp` sidecar and any
/// developed `<stem>.jpg` sitting beside it, so the frame stays whole. A
/// same-volume move is a rename; cross-volume falls back to copy+remove.
/// Refuses to overwrite an existing file at the destination (the RAW stays
/// put and an error is returned). Returns the RAW's new path — the caller
/// rescans both folders to reconcile the index.
#[tauri::command]
pub(crate) async fn move_photo(path: String, dest_dir: String) -> Result<String, String> {
    apple_photos::require_file(&path)?;
    apple_photos::require_file(&dest_dir)?;
    tauri::async_runtime::spawn_blocking(move || {
        let src = std::path::PathBuf::from(&path);
        let dest_dir = std::path::PathBuf::from(&dest_dir);
        let src_dir = src
            .parent()
            .ok_or_else(|| "photo has no parent folder".to_string())?;
        if src_dir == dest_dir {
            return Err("the photo is already in this folder".to_string());
        }
        if !dest_dir.is_dir() {
            return Err(format!(
                "destination folder not found: {}",
                dest_dir.display()
            ));
        }
        let file_name = src
            .file_name()
            .ok_or_else(|| "invalid file name".to_string())?;
        let new_path = dest_dir.join(file_name);
        if new_path.exists() {
            return Err(format!(
                "a file named \u{201c}{}\u{201d} already exists in the destination folder",
                file_name.to_string_lossy()
            ));
        }

        // A sidecar already waiting at the destination belongs to something
        // else (an orphan, or another photo's decisions). A rename would
        // replace it without a word, so refuse before anything moves.
        let sidecar = reveal_meta::sidecar_path(&src);
        let dest_sidecar = reveal_meta::sidecar_path(&new_path);
        if sidecar.exists() && dest_sidecar.exists() {
            return Err(format!(
                "a sidecar named \u{201c}{}\u{201d} already exists in the destination folder",
                dest_sidecar.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
            ));
        }

        move_one(&src, &new_path).map_err(|e| format!("move failed: {e}"))?;

        // The sidecar holds the photo's decisions: if it cannot follow, the
        // frame goes back rather than leaving them behind in the old folder.
        if sidecar.exists() {
            if let Err(e) = move_one(&sidecar, &dest_sidecar) {
                return match move_one(&new_path, &src) {
                    Ok(()) => Err(format!("could not move the sidecar, so the photo stayed where it was: {e}")),
                    Err(back) => Err(format!(
                        "could not move the sidecar ({e}), and the photo could not be put back ({back}); it is now at {}",
                        new_path.display()
                    )),
                };
            }
        }
        // The camera's JPEG travels too, whatever its capitalisation. Best
        // effort: a missing one never blocks the frame from landing.
        if let Some(stem) = src.file_stem() {
            for ext in ["jpg", "JPG", "jpeg", "JPEG"] {
                let name = format!("{}.{ext}", stem.to_string_lossy());
                let jpg = src_dir.join(&name);
                if !jpg.exists() {
                    continue;
                }
                let dest_jpg = dest_dir.join(&name);
                if !dest_jpg.exists() {
                    let _ = move_one(&jpg, &dest_jpg);
                }
                break;
            }
        }
        eprintln!("moved: {} → {}", src.display(), new_path.display());
        Ok(new_path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Validate a folder's new name: non-empty, no path separator, not `.`/`..`.
/// Folder names never need the cross-platform paranoia file names do (no
/// extension, no case-insensitive collision risk beyond the `exists()` check
/// the callers already do), so this is intentionally small.
fn validate_dir_name(name: &str) -> Result<&str, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("the name cannot be empty".to_string());
    }
    if trimmed.contains('/') || trimmed == "." || trimmed == ".." {
        return Err("invalid folder name".to_string());
    }
    Ok(trimmed)
}

/// Rename a folder in place. Carries its story note along if one exists
/// (`story.rs::note_path` derives the note's name from the folder's name, so
/// a bare directory rename would otherwise orphan it — Reveal would look for
/// `<new-name>.md` and silently find nothing). Refuses to overwrite an
/// existing folder at the new name. The caller reindexes to reconcile.
#[tauri::command]
pub(crate) async fn rename_dir(
    index: tauri::State<'_, IndexState>,
    path: String,
    new_name: String,
) -> Result<String, String> {
    let idx = index.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let src = std::path::PathBuf::from(&path);
        let name = validate_dir_name(&new_name)?;
        let old_name = src
            .file_name()
            .ok_or_else(|| "invalid folder".to_string())?
            .to_string_lossy()
            .into_owned();
        if name == old_name {
            return Ok(src.to_string_lossy().into_owned());
        }
        let parent = src
            .parent()
            .ok_or_else(|| "folder has no parent".to_string())?;
        let dest = parent.join(name);
        // On a case-insensitive volume "foo" already "exists" when the folder
        // is "Foo": that is the folder itself, and a change of case is allowed.
        if dest.exists() && !is_same_dir(&src, &dest) {
            return Err(format!("\u{201c}{name}\u{201d} already exists"));
        }
        std::fs::rename(&src, &dest).map_err(|e| format!("rename failed: {e}"))?;
        // A library root at or under this folder now lives at the new path.
        if let Err(e) = idx.relocate(&path, &dest.to_string_lossy()) {
            eprintln!("rename: index not updated: {e}");
        }

        // Best-effort: the folder rename already succeeded, so a note that
        // fails to follow along is a smaller problem than pretending the
        // whole operation failed.
        let old_note = dest.join(format!("{old_name}.md"));
        let new_note = dest.join(format!("{name}.md"));
        if old_note.exists() && !new_note.exists() {
            let _ = std::fs::rename(&old_note, &new_note);
        }

        eprintln!("renamed: {} → {}", src.display(), dest.display());
        Ok(dest.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Create a new, empty subfolder inside `parent_dir`. An empty folder has no
/// frames, so it won't appear in the index-derived sidebar tree until
/// something lands in it — the frontend keeps its own ephemeral marker so it
/// stays visible (and usable as a drop target) in the meantime.
#[tauri::command]
pub(crate) async fn create_dir(parent_dir: String, name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let name = validate_dir_name(&name)?;
        let dest = std::path::PathBuf::from(&parent_dir).join(name);
        if dest.exists() {
            return Err(format!("\u{201c}{name}\u{201d} already exists"));
        }
        std::fs::create_dir(&dest).map_err(|e| format!("creation failed: {e}"))?;
        eprintln!("created: {}", dest.display());
        Ok(dest.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Move a folder (with everything in it) to become a child of
/// `dest_parent_dir`. Same-volume only for now — a folder can hold an
/// unbounded amount of NAS-backed data, and a cross-volume recursive
/// copy+remove has a much larger partial-failure window than the single-file
/// fallback `move_photo` uses; safer to refuse than to half-move a library
/// folder. Refuses to move a folder into itself, into its own descendant, or
/// onto an existing folder of the same name.
#[tauri::command]
pub(crate) async fn move_dir(
    index: tauri::State<'_, IndexState>,
    path: String,
    dest_parent_dir: String,
) -> Result<String, String> {
    let idx = index.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let src = std::path::PathBuf::from(&path);
        let dest_parent = std::path::PathBuf::from(&dest_parent_dir);
        let name = src
            .file_name()
            .ok_or_else(|| "invalid folder".to_string())?;
        let dest = dest_parent.join(name);

        if dest_parent == src {
            return Err("a folder cannot contain itself".to_string());
        }
        if dest_parent.starts_with(&src) {
            return Err("cannot move a folder into one of its own subfolders".to_string());
        }
        if let Some(current_parent) = src.parent() {
            if current_parent == dest_parent {
                return Err("the folder is already there".to_string());
            }
        }
        if dest.exists() {
            return Err(format!(
                "a folder named \u{201c}{}\u{201d} already exists at the destination",
                name.to_string_lossy()
            ));
        }
        if !dest_parent.is_dir() {
            return Err("destination folder not found".to_string());
        }
        // A library, or a folder holding one, cannot go inside another library.
        idx.check_can_move(&path, &dest_parent_dir).map_err(|e| e.to_string())?;

        match std::fs::rename(&src, &dest) {
            Ok(()) => {
                if let Err(e) = idx.relocate(&path, &dest.to_string_lossy()) {
                    eprintln!("move: index not updated: {e}");
                }
                eprintln!("moved: {} → {}", src.display(), dest.display());
                Ok(dest.to_string_lossy().into_owned())
            }
            Err(e) => Err(format!(
                "move failed (cross-volume moves aren\u{2019}t supported for folders): {e}"
            )),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Rename within a volume; copy+remove across volumes (rename returns EXDEV).
///
/// The copy goes through a temporary file next to the destination, so a copy
/// that fails half way never leaves a truncated file under the real name (which
/// would also block every retry), and the source is only removed once the
/// copy is complete.
fn move_one(src: &std::path::Path, dest: &std::path::Path) -> std::io::Result<()> {
    if std::fs::rename(src, dest).is_ok() {
        return Ok(());
    }
    let tmp = dest.with_file_name(format!(
        ".{}.part",
        dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    ));
    let copied = std::fs::copy(src, &tmp).and_then(|n| {
        if n == std::fs::metadata(src)?.len() {
            std::fs::rename(&tmp, dest)
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::Other, "copy is shorter than the original"))
        }
    });
    if let Err(e) = copied {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    std::fs::remove_file(src)
}

/// Move a photo and its companions into `dest_dir` as one unit.
///
/// `move_one` starts with a `rename`, which silently replaces whatever sits at
/// the destination, so every target is checked first and a clash refuses the
/// whole group before anything moves. If a move fails part way, what already
/// moved goes back, so a photo is never separated from the sidecar that holds
/// its decisions.
fn move_group(
    photo: &std::path::Path,
    dest_dir: &std::path::Path,
    companions: &[std::path::PathBuf],
) -> Result<(), String> {
    let mut pairs: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    for src in std::iter::once(photo).chain(companions.iter().map(|c| c.as_path())) {
        let Some(name) = src.file_name() else { continue };
        pairs.push((src.to_path_buf(), dest_dir.join(name)));
    }
    if let Some((_, dest)) = pairs.iter().find(|(_, dest)| dest.exists()) {
        return Err(format!(
            "\u{201c}{}\u{201d} is already in the destination folder",
            dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
        ));
    }
    let mut done: Vec<&(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    for pair in &pairs {
        if let Err(e) = move_one(&pair.0, &pair.1) {
            let mut stuck = Vec::new();
            for (from, to) in done.iter().rev().map(|p| (&p.0, &p.1)) {
                if let Err(back) = move_one(to, from) {
                    stuck.push(format!("{} ({back})", to.display()));
                }
            }
            let name = pair.0.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            return Err(if stuck.is_empty() {
                format!("could not move {name}, so the photo stayed where it was: {e}")
            } else {
                format!("could not move {name} ({e}), and could not put back: {}", stuck.join(", "))
            });
        }
        done.push(pair);
    }
    Ok(())
}

/// Whether two paths are one folder. Differs from `==` on a case-insensitive
/// volume, where `Foo` and `foo` name the same place.
fn is_same_dir(a: &std::path::Path, b: &std::path::Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// The indexed folder list (with counts) + every catalogue root. The first
/// tuple element is the FULL root set now (was a single Option<String>) — the
/// sidebar renders one tree per root instead of guessing catalogues.
#[tauri::command]
pub(crate) fn index_dirs(
    index: tauri::State<'_, IndexState>,
) -> Result<(Vec<String>, Vec<reveal_index::DirRow>), String> {
    Ok((
        index.0.roots().map_err(|e| e.to_string())?,
        index.0.dirs().map_err(|e| e.to_string())?,
    ))
}

/// Register a new catalogue root and index it — the `+` in the sidebar. Adds
/// to the root set (does not evict existing libraries), then scans its subtree.
#[tauri::command]
pub(crate) async fn add_catalog_root(
    app: tauri::AppHandle,
    index: tauri::State<'_, IndexState>,
    path: String,
) -> Result<reveal_index::ScanStats, String> {
    // Libraries do not nest: refuse before registering anything.
    index.0.check_can_add_root(&path).map_err(|e| e.to_string())?;
    index.0.add_root(&path).map_err(|e| e.to_string())?;
    // `scan_root` re-registers (idempotent) and walks the tree.
    scan_root(app, index, path).await
}

/// Park the open photo's decoded frame so a restart can reopen it instantly.
///
/// Develop holds one photo; parking it costs ~33 MB of local disk and turns
/// the next launch's ~3.1s network read plus ~1.0s decode into a local read.
/// Best-effort and off the caller's thread — failing to park just means the
/// next launch pays what it pays today.
#[tauri::command]
pub(crate) async fn park_working_frame(
    state: tauri::State<'_, EngineState>,
    path: String,
) -> Result<(), String> {
    let engine = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let source = match apple_photos::source(&path) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("park working frame {path}: {e}");
                return;
            }
        };
        if let Err(e) = engine.park_working(&source, 2048) {
            eprintln!("park working frame {path}: {e:#}");
        }
    });
    Ok(())
}

/// Drop the parked frame — leaving Develop for the grid, where one photo's
/// decode is just tens of megabytes of disk doing nothing.
#[tauri::command]
pub(crate) async fn release_working_frame(state: tauri::State<'_, EngineState>) -> Result<(), String> {
    let engine = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || engine.clear_working());
    Ok(())
}

/// The file's bytes, if it is an ordinary image rather than a RAW.
///
/// Can we reach the folder this photo lives in right now?
///
/// The UI marks a photo as coming from cache while its source is unreachable;
/// this is how it learns the archive came back, without waiting for the next
/// failure to tell it.
#[tauri::command]
pub(crate) fn source_reachable(path: String) -> bool {
    let p = std::path::Path::new(&path);
    apple_photos::is_asset(&path) || crate::immich::is_asset(&path) || is_volume_mounted(p)
}

/// Every registered library, with its frame count and whether its folder is
/// reachable right now. Drives the Libraries tab in Settings.
#[tauri::command]
pub(crate) async fn catalog_roots(
    index: tauri::State<'_, IndexState>,
) -> Result<Vec<reveal_index::Catalogue>, String> {
    let idx = index.0.clone();
    tauri::async_runtime::spawn_blocking(move || idx.catalogues())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Forget a catalogue root: drop it from the set and prune its frames. The
/// files on disk are untouched — this only removes the library from the index.
#[tauri::command]
pub(crate) fn remove_catalog_root(
    index: tauri::State<'_, IndexState>,
    path: String,
) -> Result<usize, String> {
    index.0.remove_root(&path).map_err(|e| e.to_string())
}

/// Frames of one indexed folder, filtered by minimum rating.
#[tauri::command]
pub(crate) async fn index_frames(
    index: tauri::State<'_, IndexState>,
    dir: String,
    min_rating: u8,
) -> Result<Vec<reveal_index::FrameRow>, String> {
    // The frames() query walks the whole `frames` table (LIKE filters over
    // ~22k rows for "the whole library"). As a SYNC command this ran on the
    // main thread and froze the entire UI for the duration. Clone the Arc and
    // hand the blocking SQLite work to a worker so the main thread stays live.
    let idx = index.0.clone();
    let rows = tauri::async_runtime::spawn_blocking(move || idx.frames(&dir, min_rating))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    eprintln!("index_frames min={min_rating} → {} rows", rows.len());
    Ok(rows)
}

// ---------------------------------------------------------------------- tidy

/// One destination folder in a tidy plan.
#[derive(serde::Serialize)]
pub(crate) struct TidyGroup {
    to_dir: String,
    count: usize,
    /// How many of them, by reason: (reason, count).
    reasons: Vec<(String, usize)>,
    /// Where they are now: (folder, count), the biggest few.
    from_dirs: Vec<(String, usize)>,
}

#[derive(serde::Serialize)]
pub(crate) struct TidyConflict {
    from: String,
    to: String,
    why: String,
}

/// A tidy plan, shaped to be read by a person: counted, grouped, and capped so
/// a hundred thousand photos do not become a hundred thousand rows.
#[derive(serde::Serialize)]
pub(crate) struct TidyView {
    root: String,
    base: String,
    pattern: String,
    summary: reveal_import::tidy::Summary,
    groups: Vec<TidyGroup>,
    more_groups: usize,
    /// Named folders whose photos are left alone: (folder, photos).
    kept: Vec<(String, usize)>,
    conflicts: Vec<TidyConflict>,
    more_conflicts: usize,
    /// Photos with no capture date (a few names, for recognising them).
    undated: Vec<String>,
}

const TIDY_MAX_GROUPS: usize = 400;
const TIDY_MAX_ROWS: usize = 200;

fn tidy_view(plan: reveal_import::tidy::TidyPlan) -> TidyView {
    use reveal_import::tidy::{Reason, Verdict};
    use std::collections::BTreeMap;
    let name = |p: &std::path::Path| p.to_string_lossy().into_owned();
    let folder_of = |p: &std::path::Path| p.parent().map(name).unwrap_or_default();

    let mut groups: BTreeMap<String, (usize, BTreeMap<&'static str, usize>, BTreeMap<String, usize>)> = BTreeMap::new();
    let mut kept: BTreeMap<String, usize> = BTreeMap::new();
    let mut conflicts = Vec::new();
    let mut more_conflicts = 0usize;
    let mut undated = Vec::new();

    for item in &plan.items {
        match &item.verdict {
            Verdict::Move { reason } => {
                let to = item.to.as_deref().map(folder_of).unwrap_or_default();
                let g = groups.entry(to).or_default();
                g.0 += 1;
                let r = match reason {
                    Reason::WrongPlace => "right day, wrong place",
                    Reason::WrongDay => "filed under another day",
                    Reason::NotFiled => "not in a day folder",
                };
                *g.1.entry(r).or_default() += 1;
                *g.2.entry(folder_of(&item.from)).or_default() += 1;
            }
            Verdict::Kept { folder } => *kept.entry(folder.clone()).or_default() += 1,
            Verdict::Conflict { why } => {
                if conflicts.len() < TIDY_MAX_ROWS {
                    conflicts.push(TidyConflict {
                        from: name(&item.from),
                        to: item.to.as_deref().map(name).unwrap_or_default(),
                        why: why.clone(),
                    });
                } else {
                    more_conflicts += 1;
                }
            }
            Verdict::Undated => {
                if undated.len() < TIDY_MAX_ROWS {
                    undated.push(name(&item.from));
                }
            }
            Verdict::InPlace => {}
        }
    }

    let total_groups = groups.len();
    let groups: Vec<TidyGroup> = groups
        .into_iter()
        .take(TIDY_MAX_GROUPS)
        .map(|(to_dir, (count, reasons, from))| {
            let mut from_dirs: Vec<(String, usize)> = from.into_iter().collect();
            from_dirs.sort_by(|a, b| b.1.cmp(&a.1));
            from_dirs.truncate(3);
            TidyGroup {
                to_dir,
                count,
                reasons: reasons.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
                from_dirs,
            }
        })
        .collect();

    let mut kept: Vec<(String, usize)> = kept.into_iter().collect();
    kept.sort_by(|a, b| b.1.cmp(&a.1));

    TidyView {
        root: name(&plan.root),
        base: name(&plan.base),
        pattern: plan.pattern.clone(),
        summary: plan.summary.clone(),
        more_groups: total_groups.saturating_sub(groups.len()),
        groups,
        kept,
        conflicts,
        more_conflicts,
        undated,
    }
}

/// What it would take to file `dir`'s photos by the import rule, as a plan.
/// Reads only: no file is moved, written or deleted.
///
/// The rule's folders live under `base`: the import folder when there is one,
/// else the library that holds `dir`. Dates come from the index when it has them
/// and from the files when it does not. Progress goes out as `tidy-progress`.
#[tauri::command]
pub(crate) async fn tidy_plan(
    app: tauri::AppHandle,
    index: tauri::State<'_, IndexState>,
    dir: String,
) -> Result<TidyView, String> {
    let idx = index.0.clone();
    let library = idx
        .library_covering(&dir)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "This folder is not inside any of your libraries, so there is nothing to tidy it into.".to_string())?;
    let pattern = load_preferences(app.clone())
        .get("date_folders")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(reveal_import::DEFAULT_DATE_FORMAT)
        .to_string();
    // The archive the import files into, if it is inside a library; else the library itself.
    let base = read_shell_prefs(&app)
        .import_dir
        .filter(|d| idx.library_covering(d).ok().flatten().is_some() && std::path::Path::new(d).is_dir())
        .unwrap_or(library);

    tauri::async_runtime::spawn_blocking(move || {
        let root = std::path::PathBuf::from(&dir);
        let (raws, others) = reveal_import::tidy::find_photos(&root);
        let dates = idx.capture_times_under(&dir).unwrap_or_default();
        let progress_app = app.clone();
        let photos = reveal_import::tidy::read_photos(
            raws,
            &|p: &std::path::Path| dates.get(p.to_string_lossy().as_ref()).copied(),
            &move |done, total| {
                let _ = progress_app.emit("tidy-progress", serde_json::json!({ "done": done, "total": total }));
            },
        );
        let cfg = reveal_import::tidy::Config {
            root: &root,
            base: std::path::Path::new(&base),
            pattern: &pattern,
            now: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        };
        let plan = reveal_import::tidy::plan(&cfg, photos, others, &|p: &std::path::Path| p.exists());
        let _ = app.emit("tidy-progress", serde_json::json!({ "done": true }));
        Ok(tidy_view(plan))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(serde::Serialize)]
pub(crate) struct TidyApplyResult {
    pub moved: usize,
    pub errors: Vec<String>,
}

/// Applies a tidy plan on `dir`: moves photos into their planned day folders,
/// carries sidecars and companions along, and refreshes the catalog index.
#[tauri::command]
pub(crate) async fn tidy_apply(
    app: tauri::AppHandle,
    index: tauri::State<'_, IndexState>,
    dir: String,
) -> Result<TidyApplyResult, String> {
    let idx = index.0.clone();
    let library = idx
        .library_covering(&dir)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "This folder is not inside any of your libraries.".to_string())?;
    let pattern = load_preferences(app.clone())
        .get("date_folders")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(reveal_import::DEFAULT_DATE_FORMAT)
        .to_string();
    let base = read_shell_prefs(&app)
        .import_dir
        .filter(|d| idx.library_covering(d).ok().flatten().is_some() && std::path::Path::new(d).is_dir())
        .unwrap_or(library);

    tauri::async_runtime::spawn_blocking(move || {
        let root = std::path::PathBuf::from(&dir);
        let (raws, others) = reveal_import::tidy::find_photos(&root);
        let dates = idx.capture_times_under(&dir).unwrap_or_default();
        let photos = reveal_import::tidy::read_photos(
            raws,
            &|p: &std::path::Path| dates.get(p.to_string_lossy().as_ref()).copied(),
            &|_done, _total| {},
        );
        let cfg = reveal_import::tidy::Config {
            root: &root,
            base: std::path::Path::new(&base),
            pattern: &pattern,
            now: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        };
        let plan = reveal_import::tidy::plan(&cfg, photos, others, &|p: &std::path::Path| p.exists());

        use reveal_import::tidy::Verdict;
        let to_move: Vec<_> = plan
            .items
            .into_iter()
            .filter(|i| matches!(i.verdict, Verdict::Move { .. }))
            .collect();
        let total = to_move.len();

        let mut moved = 0usize;
        let mut errors = Vec::new();
        let mut touched_dirs: std::collections::HashSet<std::path::PathBuf> =
            std::collections::HashSet::new();

        for item in to_move {
            let Some(dest_file) = item.to else { continue };
            let Some(dest_dir) = dest_file.parent() else { continue };

            if let Err(e) = std::fs::create_dir_all(dest_dir) {
                errors.push(format!("could not create directory {}: {e}", dest_dir.display()));
                continue;
            }

            if let Some(src_parent) = item.from.parent() {
                touched_dirs.insert(src_parent.to_path_buf());
            }
            touched_dirs.insert(dest_dir.to_path_buf());

            // The photo and everything that travels with it, or nothing.
            let companions = reveal_import::tidy::companions_of(&item.from);
            if let Err(e) = move_group(&item.from, dest_dir, &companions) {
                errors.push(format!("{}: {e}", item.from.display()));
                continue;
            }

            moved += 1;
            let _ = app.emit(
                "tidy-apply-progress",
                serde_json::json!({
                    "done": moved,
                    "total": total,
                    "current": item.from.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                }),
            );
        }

        // Reconcile index for each affected folder
        for d in &touched_dirs {
            let _ = idx.scan_with(d, |_, _| {});
        }

        let _ = app.emit("libraries-changed", serde_json::json!({}));
        let _ = app.emit(
            "tidy-apply-progress",
            serde_json::json!({ "done": total, "total": total, "finished": true }),
        );

        Ok(TidyApplyResult { moved, errors })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("reveal-catalog-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_folder_is_the_same_folder_under_a_different_case_only_where_the_volume_says_so() {
        let dir = scratch("same-dir");
        let foo = dir.join("Foo");
        std::fs::create_dir(&foo).unwrap();
        // True on a case-insensitive volume (macOS default), false elsewhere:
        // either way it must agree with whether the other spelling exists at all.
        assert_eq!(is_same_dir(&foo, &dir.join("foo")), dir.join("foo").exists());
        assert!(is_same_dir(&foo, &foo));
        let other = dir.join("Other");
        std::fs::create_dir(&other).unwrap();
        assert!(!is_same_dir(&foo, &other));
    }

    #[test]
    fn a_group_moves_the_photo_with_its_companions() {
        let dir = scratch("group-ok");
        let (from, to) = (dir.join("from"), dir.join("to"));
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(&to).unwrap();
        let (raw, xmp) = (from.join("a.raf"), from.join("a.raf.xmp"));
        std::fs::write(&raw, b"raw").unwrap();
        std::fs::write(&xmp, b"xmp").unwrap();
        move_group(&raw, &to, &[xmp.clone()]).unwrap();
        assert!(!raw.exists() && !xmp.exists());
        assert_eq!(std::fs::read(to.join("a.raf")).unwrap(), b"raw");
        assert_eq!(std::fs::read(to.join("a.raf.xmp")).unwrap(), b"xmp");
    }

    #[test]
    fn a_group_never_overwrites_and_moves_nothing_when_a_target_is_taken() {
        let dir = scratch("group-clash");
        let (from, to) = (dir.join("from"), dir.join("to"));
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(&to).unwrap();
        let (raw, xmp) = (from.join("a.raf"), from.join("a.raf.xmp"));
        std::fs::write(&raw, b"raw").unwrap();
        std::fs::write(&xmp, b"mine").unwrap();
        std::fs::write(to.join("a.raf.xmp"), b"someone else's decisions").unwrap();
        assert!(move_group(&raw, &to, &[xmp.clone()]).is_err());
        assert!(raw.exists() && xmp.exists(), "nothing may have moved");
        assert!(!to.join("a.raf").exists());
        assert_eq!(std::fs::read(to.join("a.raf.xmp")).unwrap(), b"someone else's decisions");
    }

    #[test]
    fn a_group_puts_the_photo_back_when_a_companion_cannot_follow() {
        let dir = scratch("group-rollback");
        let (from, to) = (dir.join("from"), dir.join("to"));
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(&to).unwrap();
        let raw = from.join("a.raf");
        std::fs::write(&raw, b"raw").unwrap();
        // A companion that vanished between planning and moving.
        let ghost = from.join("a.raf.xmp");
        let err = move_group(&raw, &to, &[ghost]).unwrap_err();
        assert!(err.contains("stayed where it was"), "{err}");
        assert_eq!(std::fs::read(&raw).unwrap(), b"raw", "the photo is back where it was");
        assert!(!to.join("a.raf").exists());
    }

    #[test]
    fn moving_a_file_leaves_no_temporary_behind_and_keeps_the_bytes() {
        let dir = scratch("move-one");
        let src = dir.join("a.raf");
        std::fs::write(&src, b"original bytes").unwrap();
        let dest = dir.join("b.raf");
        move_one(&src, &dest).unwrap();
        assert!(!src.exists());
        assert_eq!(std::fs::read(&dest).unwrap(), b"original bytes");
        assert!(!dir.join(".b.raf.part").exists());
    }

    #[test]
    fn a_move_that_fails_leaves_the_source_alone() {
        let dir = scratch("move-fail");
        let src = dir.join("a.raf");
        std::fs::write(&src, b"x").unwrap();
        let dest = dir.join("missing-folder").join("a.raf");
        assert!(move_one(&src, &dest).is_err());
        assert!(src.exists(), "the original is only removed after a complete copy");
    }
}
