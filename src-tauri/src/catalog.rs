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
    let mut frames = Vec::new();
    for e in std::fs::read_dir(&path).map_err(|e| e.to_string())? {
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
        let rating = reveal_meta::read(&p)
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
#[tauri::command]
pub(crate) async fn set_rating(
    index: tauri::State<'_, IndexState>,
    path: String,
    rating: u8,
) -> Result<(), String> {
    apple_photos::update_metadata(&path, |sidecar| {
        sidecar.rating = Some(rating.min(5));
        Ok(())
    })?;
    if apple_photos::is_asset(&path) { return Ok(()); }
    index.0.set_rating(&path, rating.min(5)).map_err(|e| e.to_string())
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
                "scan {path}: {} frames ({} nouveaux, {} retirés), {} dossiers, {} ms",
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
        .root_containing(&path)
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

        // The RAW must move; its companions are best-effort so a missing
        // sidecar or jpg never blocks the frame from landing.
        move_one(&src, &new_path).map_err(|e| format!("move failed: {e}"))?;

        let sidecar = reveal_meta::sidecar_path(&src);
        if sidecar.exists() {
            let _ = move_one(&sidecar, &reveal_meta::sidecar_path(&new_path));
        }
        if let Some(stem) = src.file_stem() {
            let jpg = src_dir.join(format!("{}.jpg", stem.to_string_lossy()));
            if let Some(name) = jpg.file_name() {
                let dest_jpg = dest_dir.join(name);
                if jpg.exists() && !dest_jpg.exists() {
                    let _ = move_one(&jpg, &dest_jpg);
                }
            }
        }
        eprintln!("déplacé: {} → {}", src.display(), new_path.display());
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
pub(crate) async fn rename_dir(path: String, new_name: String) -> Result<String, String> {
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
        if dest.exists() {
            return Err(format!("\u{201c}{name}\u{201d} already exists"));
        }
        std::fs::rename(&src, &dest).map_err(|e| format!("rename failed: {e}"))?;

        // Best-effort: the folder rename already succeeded, so a note that
        // fails to follow along is a smaller problem than pretending the
        // whole operation failed.
        let old_note = dest.join(format!("{old_name}.md"));
        let new_note = dest.join(format!("{name}.md"));
        if old_note.exists() && !new_note.exists() {
            let _ = std::fs::rename(&old_note, &new_note);
        }

        eprintln!("renommé: {} → {}", src.display(), dest.display());
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
        eprintln!("créé: {}", dest.display());
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
pub(crate) async fn move_dir(path: String, dest_parent_dir: String) -> Result<String, String> {
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

        match std::fs::rename(&src, &dest) {
            Ok(()) => {
                eprintln!("déplacé: {} → {}", src.display(), dest.display());
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
fn move_one(src: &std::path::Path, dest: &std::path::Path) -> std::io::Result<()> {
    match std::fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(src, dest)?;
            std::fs::remove_file(src)?;
            Ok(())
        }
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
    index.0.add_root(&path).map_err(|e| e.to_string())?;
    // `scan_root` sets meta.root + re-registers (idempotent) and walks the tree.
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
    apple_photos::is_asset(&path) || is_volume_mounted(p)
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
    // ~22k rows for "toute la bibliothèque"). As a SYNC command this ran on the
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
