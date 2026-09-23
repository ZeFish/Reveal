//! Everything a photo's on-disk derivatives are made of: the `.preview.jpg`
//! sidecar, the local render cache and its pruning, the thumbnail protocol
//! path, and the develop commands that publish them.
//!
//! This is where every artefact bug of 21–22 September 2026 lived. Naming the
//! place is the point: the contract written on `write_preview_sidecar_bytes`
//! only means anything if the other writers are within sight of it.
//!
//! Lifted out of `lib.rs` unchanged.

use crate::*;

/// Develop a RAW through the film pipeline, return JPEG bytes (binary IPC —
/// zero JSON, zero base64). `max_px` bounds the preview's long edge.
pub(crate) fn developed_preview_source_key(path: &std::path::Path) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn developed_preview_cache_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| e.to_string())?
        .join("DevelopPreviews");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// The durable developed preview, kept as a sibling of the source file
/// (`DSCF3098.RAF` → `DSCF3098.preview.jpg`). This is the "file over app"
/// truth: a 2048px developed JPEG that doubles as a web-ready export. The
/// app-cache variants under DevelopPreviews/ are only a speed layer.
pub(crate) fn preview_sidecar_path(source: &std::path::Path) -> Option<std::path::PathBuf> {
    if apple_photos::is_asset(&source.to_string_lossy()) {
        return match apple_photos::metadata_path(&source.to_string_lossy()) {
            Ok(path) => preview_sidecar_path(&path),
            Err(error) => {
                eprintln!("Apple Photos preview path: {error}");
                None
            }
        };
    }
    source
        .file_stem()
        .map(|stem| source.with_file_name(format!("{}.preview.jpg", stem.to_string_lossy())))
}

/// A real camera-written JPEG sitting next to the RAW (RAW+JPEG shooting).
/// This is the camera's own full-resolution render — correct even when it
/// diverges from a plain color decode (a monochrome film simulation, for
/// instance: the sensor data is always color, but the camera's OWN JPEG
/// correctly reflects what the photographer intended). Checked AFTER the
/// embedded-thumb extraction (only when a RAW has no embedded thumb to fall
/// back on cheaply) — it's full camera resolution, tens of MB, and decoding
/// it for every grid cell in a RAW+JPEG folder is what spiked memory into
/// the tens of GB before this was reordered (2026-08-02).
pub(crate) fn companion_jpeg_path(source: &std::path::Path) -> Option<std::path::PathBuf> {
    let stem = source.file_stem()?.to_string_lossy().to_string();
    ["JPG", "jpg", "JPEG", "jpeg"]
        .iter()
        .map(|ext| source.with_file_name(format!("{stem}.{ext}")))
        .find(|candidate| candidate.is_file())
}

/// Shrink a served thumbnail to `max_edge` so the webview decodes a small
/// bitmap. Only touches what's sent over the protocol — never the on-disk
/// preview. Returns the original bytes unchanged if it's already small or
/// can't be decoded, so a weird source can never turn into a broken thumb.
///
/// `max_edge` is the caller's requested size, not a constant: the contact
/// sheet shows ~120 cells at once and a 2048px JPEG each meant the webview
/// held ~1.4 GB of decoded bitmaps and froze on scroll — but a single-photo
/// view has exactly one, and blowing a 640px proxy up to fill the window
/// while the RAW decodes is a worse picture than the 2048px `.preview.jpg`
/// already sitting on disk at the very recipe being displayed.
///
/// `source_orientation` is what the file the bytes came from says (an EXIF
/// Orientation value; 1 when there is no such file, or it is the JPEG
/// itself). The JPEG's own tag wins when it has one; when it is silent —
/// a RAW's embedded preview often is — the source's applies. A required
/// parameter, not a default, so no call site can forget to pass it.
pub(crate) fn downscale_grid_thumb(bytes: Vec<u8>, max_edge: u32, source_orientation: u32) -> Vec<u8> {
    let own = exif::Reader::new()
        .read_from_container(&mut std::io::Cursor::new(&bytes))
        .ok()
        .and_then(|exif_data| {
            exif_data
                .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                .and_then(|field| field.value.get_uint(0))
        })
        .filter(|v| (1..=8).contains(v));
    let orientation = own.unwrap_or(source_orientation);

    let mut img = match image::load_from_memory(&bytes) {
        Ok(img) => img,
        Err(_) => return bytes,
    };

    // Apply EXIF orientation before resizing so the downscaled grid thumb is correctly rotated
    img = match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate90().flipv(),
        8 => img.rotate270(),
        _ => img,
    };

    if img.width() <= max_edge && img.height() <= max_edge {
        let mut out = std::io::Cursor::new(Vec::new());
        return match img.write_to(&mut out, image::ImageFormat::Jpeg) {
            Ok(()) => out.into_inner(),
            Err(_) => bytes,
        };
    }
    // `thumbnail` keeps aspect ratio and uses a fast filter — right for grid cells.
    let small = img.thumbnail(max_edge, max_edge);
    let mut out = std::io::Cursor::new(Vec::new());
    match small.write_to(&mut out, image::ImageFormat::Jpeg) {
        Ok(()) => out.into_inner(),
        Err(_) => bytes,
    }
}

/// mtime (ms since epoch) of the developed sidecar the `thumb` protocol would
/// serve — `.preview.jpg`, else 0 (as-shot). The frontend uses it as the thumb
/// URL's cache-busting version, so an external edit to the sidecar surfaces in
/// the grid on the next load — file over app.
pub(crate) fn served_preview_mtime(source: &std::path::Path) -> u64 {
    if !is_volume_mounted(source) {
        return 0;
    }
    if let Some(candidate) = preview_sidecar_path(source) {
        if let Ok(ms) = std::fs::metadata(&candidate)
            .and_then(|m| m.modified())
            .map(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64
            })
        {
            return ms;
        }
    }
    0
}

/// Write the durable sidecar only when the bytes actually differ, so merely
/// re-opening a developed photo never bumps the file's mtime — keeping backups
/// quiet and the future staleness check honest.
pub(crate) fn write_sidecar_if_changed(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Ok(existing) = std::fs::read(path) {
        if existing == bytes {
            return Ok(());
        }
    }
    // An Apple Photos preview lands inside a per-asset directory that nothing
    // creates on the read path any more — writing owns making room.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)
}

/// Persist a `reveal://thumb` render as the durable `.preview.jpg` sidecar so
/// the NEXT request for this photo hits the cheap "developed sidecar" branch
/// instead of re-decoding the source (embedded thumb, companion JPEG, or a
/// full develop) every single time the grid loads it. Never touches the
/// `.xmp` recipe/engine metadata — a photo with no saved develop settings
/// still reads as engine "None" if reopened; this only caches rendered bytes.
/// The size `.preview.jpg` is defined to be. The sidecar is a durable
/// artifact — publish and export both read it — so only a response rendered
/// at this size may be written there.
pub(crate) const DURABLE_PREVIEW_EDGE: u32 = 2048;

/// Persist a served thumbnail as the durable `.preview.jpg`, but ONLY when it
/// was rendered at the size that file is defined to hold.
///
/// The protocol serves the size its caller asked for — 640 for contact-sheet
/// cells, 2048 for a single-photo view — and both used to land here. That
/// made the grid and the viewer overwrite each other's sidecar on every
/// visit, rewriting the file back and forth across the network mount, and
/// left whichever came last as the "durable" preview that publish would
/// upload. Serving small and persisting small are different decisions.
pub(crate) fn persist_thumb_cache(source: &std::path::Path, bytes: &[u8], size: u32) {
    if size < DURABLE_PREVIEW_EDGE {
        return;
    }
    if let Some(sidecar_path) = preview_sidecar_path(source) {
        if let Err(e) = write_sidecar_if_changed(&sidecar_path, bytes) {
            eprintln!("thumb cache write {}: {e}", sidecar_path.display());
        }
    }
}

/// Bounds how many `reveal://thumb` decodes run at once. Without this, a
/// folder opened all-at-once (masonry isn't grid-virtualized yet, per the
/// README) can spawn one blocking decode per visible cell — fine for the
/// cheap embedded-thumb path, but the companion-JPEG/full-develop fallbacks
/// are expensive enough that hundreds running concurrently spikes memory
/// into the tens of GB (confirmed 2026-08-02 opening a 213-photo RAW+JPEG
/// folder — see reveal.md memory notes).
pub(crate) struct ThumbSemaphore {
    count: std::sync::Mutex<usize>,
    cv: std::sync::Condvar,
    max: usize,
}

impl ThumbSemaphore {
    pub(crate) fn new(max: usize) -> Self {
        Self { count: std::sync::Mutex::new(0), cv: std::sync::Condvar::new(), max }
    }

    pub(crate) fn acquire(&self) {
        let mut count = self.count.lock().unwrap();
        while *count >= self.max {
            count = self.cv.wait(count).unwrap();
        }
        *count += 1;
    }

    fn release(&self) {
        let mut count = self.count.lock().unwrap();
        *count -= 1;
        self.cv.notify_one();
    }
}

pub(crate) struct ThumbConcurrencyState(pub(crate) std::sync::Arc<ThumbSemaphore>);

/// RAII guard: acquired before a thumb decode, released (even on early
/// return/panic-unwind) when the request finishes.
pub(crate) struct ThumbPermit<'a>(&'a ThumbSemaphore);

impl<'a> ThumbPermit<'a> {
    pub(crate) fn acquire(sem: &'a ThumbSemaphore) -> Self {
        sem.acquire();
        Self(sem)
    }
}

impl Drop for ThumbPermit<'_> {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// How much disk the develop cache may use.
///
/// A budget, not a photo count. Entries range from ~27 KB at grid size to
/// ~420 KB at 2048, so "2000 photos" stopped describing anything once the
/// cache held both: the same number meant sixty megabytes or eight hundred
/// depending on what happened to be in it.
///
/// Four gigabytes covers a whole library rather than a slice of one: measured
/// here, 105,867 indexed photos at ~36 KB each is ~3.6 GiB. That is the point
/// — the cache is what makes moving through the library fast, so its ceiling
/// should be the library. Same figure the Apple Photos source cache already
/// defaults to.
///
/// The durable truth is always the `.preview.jpg` sibling of the RAW; this
/// is a pure speed layer and evicting from it costs a NAS read, nothing more.
pub(crate) const PREVIEW_CACHE_BUDGET_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// Coalesces concurrent prune requests: a render storm schedules at most one
/// running prune at a time instead of one per frame.
pub(crate) static CACHE_PRUNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Fire-and-forget the LRU prune off the render path, guarded so overlapping
/// renders never stack prunes.
pub(crate) fn schedule_cache_prune(app: &tauri::AppHandle) {
    use std::sync::atomic::Ordering;
    if CACHE_PRUNING.swap(true, Ordering::AcqRel) {
        return;
    }
    let dir = match developed_preview_cache_dir(app) {
        Ok(d) => d,
        Err(_) => {
            CACHE_PRUNING.store(false, Ordering::Release);
            return;
        }
    };
    tauri::async_runtime::spawn_blocking(move || {
        prune_preview_cache(&dir, PREVIEW_CACHE_BUDGET_BYTES);
        CACHE_PRUNING.store(false, Ordering::Release);
    });
}

/// Keep the develop cache to one JPEG per photo, for the newest `limit`
/// photos (LRU by mtime).
///
/// Two separate trims, and the per-photo one is the important half. The cache
/// exists to make moving through the last couple of thousand photos instant —
/// not to remember old slider positions. But its key includes the recipe, so
/// every settled edit wrote another file and nothing removed the previous
/// one: measured on Francis's cache, 548 files for 99 photos, one photo
/// holding 97 stale renders of itself. Only the current recipe can ever be
/// asked for again, so only the newest survives.
///
/// The size trim used to be a photo count and to return early whenever the
/// library was under it, which meant that on any normal cache — 99 photos
/// against a limit of 2000 — nothing was ever cleaned at all. The per-entry
/// trim runs unconditionally for that reason.
pub(crate) fn prune_preview_cache(dir: &std::path::Path, budget_bytes: u64) {
    use std::collections::HashMap;
    let epoch = std::time::SystemTime::UNIX_EPOCH;
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return,
    };

    // Group every cached render by the photo it came from.
    let mut groups: HashMap<String, Vec<(std::time::SystemTime, std::path::PathBuf)>> =
        HashMap::new();
    for entry in read.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) != Some("jpg") {
            continue;
        }
        // Group by photo AND size: `{photo}-{size}-{version}.jpg`. Grouping
        // by photo alone would make a 768 entry and a 2048 entry of the same
        // photo evict each other, so the grid and the viewer would keep
        // knocking the other's copy out.
        let key = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => {
                let mut parts = name.splitn(3, '-');
                match (parts.next(), parts.next()) {
                    (Some(photo), Some(size)) => format!("{photo}-{size}"),
                    _ => name.to_string(),
                }
            }
            None => continue,
        };
        let mtime = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(epoch);
        groups.entry(key).or_default().push((mtime, path));
    }

    // One render per photo per size: drop every version but the most recent.
    let mut newest: Vec<(std::time::SystemTime, std::path::PathBuf)> =
        Vec::with_capacity(groups.len());
    for (_, mut variants) in groups {
        variants.sort_by(|a, b| b.0.cmp(&a.0)); // newest first
        let mut keep = variants.into_iter();
        if let Some(survivor) = keep.next() {
            for (_, stale) in keep {
                let _ = std::fs::remove_file(stale);
            }
            newest.push(survivor);
        }
    }

    // Then the budget: newest first, keep until the disk allowance runs out.
    // Evicting costs one NAS read on the next visit and nothing else, so the
    // only thing to get right is the ORDER — least recently touched goes.
    newest.sort_by(|a, b| b.0.cmp(&a.0));
    let mut used: u64 = 0;
    for (_, path) in newest {
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if used + size <= budget_bytes {
            used += size;
        } else {
            let _ = std::fs::remove_file(path);
        }
    }
}

/// Rendered-JPEG speed cache, distinct from the Apple Photos cache (which
/// holds downloaded RAW originals, source-availability, not a render
/// shortcut). Count-based, not size-based — no configurable limit here,
/// unlike Apple Photos' GiB setting, so this status has no `limit_bytes`.
#[derive(serde::Serialize)]
pub(crate) struct PreviewCacheStatus {
    size_bytes: u64,
    photo_count: usize,
    /// The disk allowance. The photo count is still reported because it is
    /// what a photographer thinks in, but it is an OUTCOME now, not a limit:
    /// how many photos fit depends on their size.
    limit_bytes: u64,
}

#[derive(serde::Serialize)]
pub(crate) struct PreviewCacheCleanup {
    removed_bytes: u64,
}

#[tauri::command]
pub(crate) async fn developed_preview_cache_status(app: tauri::AppHandle) -> Result<PreviewCacheStatus, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = developed_preview_cache_dir(&app)?;
        let mut size_bytes = 0u64;
        let mut keys = std::collections::HashSet::new();
        if let Ok(read) = std::fs::read_dir(&dir) {
            for entry in read.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().and_then(|x| x.to_str()) != Some("jpg") {
                    continue;
                }
                if let Ok(meta) = entry.metadata() {
                    size_bytes += meta.len();
                }
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    keys.insert(name.split('-').next().unwrap_or(name).to_string());
                }
            }
        }
        Ok(PreviewCacheStatus {
            size_bytes,
            photo_count: keys.len(),
            limit_bytes: PREVIEW_CACHE_BUDGET_BYTES,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) async fn developed_preview_cache_clear(app: tauri::AppHandle) -> Result<PreviewCacheCleanup, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = developed_preview_cache_dir(&app)?;
        let mut removed_bytes = 0u64;
        if let Ok(read) = std::fs::read_dir(&dir) {
            for entry in read.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().and_then(|x| x.to_str()) != Some("jpg") {
                    continue;
                }
                if let Ok(meta) = entry.metadata() {
                    removed_bytes += meta.len();
                }
                let _ = std::fs::remove_file(path);
            }
        }
        Ok(PreviewCacheCleanup { removed_bytes })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Copy bytes we already hold into the local cache.
///
/// One of exactly two writers (see `write_preview_sidecar_bytes` for the
/// other, which PUBLISHES renders). This one only ever MIRRORS: it stores
/// what was just read from the NAS sidecar or lifted off a card, never a
/// render of its own. Keeping those two roles apart is what stops the cache
/// and the sidecar from drifting.
///
/// Best-effort throughout — a cache that fails to fill costs the next visit
/// one NAS read and nothing else, so no caller checks the result. Temp file
/// plus rename means a half-written entry can never be served as a photo.
pub(crate) fn cache_developed_preview_locally(
    app: &tauri::AppHandle,
    source: &std::path::Path,
    bytes: &[u8],
    max_px: u32,
    version: u64,
) {
    let Ok(dest) = developed_preview_cache_path(app, source, max_px, version) else {
        return;
    };
    let tmp = dest.with_extension("part");
    if std::fs::write(&tmp, bytes).is_ok() && std::fs::rename(&tmp, &dest).is_ok() {
        schedule_cache_prune(app);
    } else {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// The size the grid actually asks for. Entries at this size are what the
/// cache is FOR — at ~27 KB against ~420 KB for a 2048, the same disk holds
/// roughly fourteen times as many photos.
pub(crate) const GRID_PREVIEW_EDGE: u32 = 768;

/// Where a photo's local render lives: photo, size, version.
///
/// No recipe in the key — the cache is the `.preview.jpg` kept locally
/// (Francis: "la cache est le .preview.jpg mais local"), and the path has to
/// be computable from a navigation request, which knows a path and a size and
/// never a recipe.
///
/// `version` is the sidecar's mtime. It carries the "file over app" guarantee
/// into the cache: edit `.preview.jpg` outside Reveal and the key moves, so
/// the stale entry is not found and the change surfaces. Without it the cache
/// would answer first and hide the edit forever.
pub(crate) fn developed_preview_cache_path(
    app: &tauri::AppHandle,
    path: &std::path::Path,
    max_px: u32,
    version: u64,
) -> Result<std::path::PathBuf, String> {
    let dir = developed_preview_cache_dir(app)?;
    Ok(dir.join(developed_preview_cache_name(path, max_px, version)))
}

/// The newest cached render of a photo at a size, whatever version it carries.
///
/// The exact key needs the sidecar's mtime, which needs the NAS. Offline —
/// on a train, in a café — that lookup returns 0 and matches nothing, so a
/// cache full of this library's photos would sit there unusable. When the
/// volume is unreachable there is no newer truth to be had, so the newest
/// thing on this disk IS the answer.
///
/// Only ever consulted while the volume is DOWN. Online, an exact miss must
/// go to the NAS, or an edit made elsewhere would never surface.
pub(crate) fn newest_cached_render(
    app: &tauri::AppHandle,
    path: &std::path::Path,
    max_px: u32,
) -> Option<std::path::PathBuf> {
    let dir = developed_preview_cache_dir(app).ok()?;
    let prefix = format!("{:016x}-{max_px}-", developed_preview_source_key(path));
    newest_matching(&dir, &prefix)
}

/// Newest `.jpg` in `dir` whose name starts with `prefix`.
///
/// Split from `newest_cached_render` so the selection can be tested without a
/// running Tauri app — the prefix carries both the photo and the size, and
/// getting either wrong would serve one photo's pixels for another.
pub(crate) fn newest_matching(dir: &std::path::Path, prefix: &str) -> Option<std::path::PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with(prefix) && n.ends_with(".jpg"))
        })
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}

/// `{photo}-{size}-{version}.jpg`. Built here, taken apart by
/// `prune_preview_cache`, which groups on the first two fields — so the
/// format has one definition and a test that round-trips it.
pub(crate) fn developed_preview_cache_name(path: &std::path::Path, max_px: u32, version: u64) -> String {
    format!(
        "{:016x}-{max_px}-{version:x}.jpg",
        developed_preview_source_key(path)
    )
}

/// Shared by `develop_preview` (returns bytes to the frontend) and
/// `copy_developed_preview_to_clipboard` (writes bytes straight to
/// NSPasteboard) — same cache-or-develop logic either way.
pub(crate) async fn developed_preview_jpeg(
    app: &tauri::AppHandle,
    state: &EngineState,
    path: &str,
    recipe: &reveal_engine::Recipe,
    max_px: u32,
) -> Result<Vec<u8>, String> {
    // The durable truth is the `.preview.jpg` sibling of the RAW (file over
    // app), a 2048px develop that doubles as a web-ready export. Only the
    // settled full-res render persists.
    let durable = max_px >= 2048;

    // No cache read here, deliberately. This is called to PRODUCE a render of
    // the recipe it was handed; the cache holds whatever was published last,
    // which is a different question. Answering it needed a recipe digest
    // stored beside every entry, and forgetting that check on one of the two
    // write paths is exactly what shipped the stale-sidecar bug. The frontend
    // is already showing `.preview.jpg` by the time it calls this, so the
    // render it wants is a new one.

    let engine = state.0.clone();
    // Clone for the blocking closure so the FULL path survives the move — the
    // sidecar funnel below needs it. Passing only the filename here wrote
    // `DSCF….preview.jpg` into the process CWD (the repo root during dev)
    // instead of next to the RAW, so the grid never saw fresh develops.
    let path_owned = path.to_string();
    let recipe_owned = recipe.clone();
    let out = tauri::async_runtime::spawn_blocking(move || {
        let source = apple_photos::source(&path_owned)?;
        engine.develop_jpeg(&source, &recipe_owned, max_px).map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))?;

    eprintln!(
        "develop_preview: {}x{} decode {} ms pipeline {} ms",
        out.width, out.height, out.decode_ms, out.render_ms
    );
    // One call publishes to both stores — see the contract on this function.
    write_preview_sidecar_bytes(app, path, &out.jpeg, durable, max_px);
    Ok(out.jpeg)
}

#[tauri::command]
pub(crate) async fn develop_preview(
    app: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
    path: String,
    recipe: reveal_engine::Recipe,
    max_px: u32,
) -> Result<IpcResponse, String> {
    let jpeg = developed_preview_jpeg(&app, &state, &path, &recipe, max_px).await?;
    Ok(IpcResponse::new(jpeg))
}

/// ⌘C in single-photo mode, for every non-Rapid engine (whose on-screen
/// preview is a decoded JPEG, not a live canvas) — writes straight to
/// NSPasteboard instead of going through the WebView's Clipboard API.
/// Reading the on-screen preview back via `<img>`/`canvas.toBlob()` hit two
/// separate WebKit bugs in a row (fetch() on the app's own blob: URLs
/// throwing "Load failed"; then the resulting canvas read as tainted,
/// throwing SecurityError) — reproduced 2026-08-02. Going fully native side-
/// steps the whole WebView canvas/blob/CORS category instead of chasing a
/// third WebKit quirk.
#[tauri::command]
pub(crate) async fn copy_developed_preview_to_clipboard(
    app: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
    path: String,
    recipe: reveal_engine::Recipe,
    max_px: u32,
) -> Result<(), String> {
    let jpeg = developed_preview_jpeg(&app, &state, &path, &recipe, max_px).await?;
    #[cfg(target_os = "macos")]
    {
        return macos::clipboard::write_jpeg_image(&jpeg);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = jpeg;
        Err("copy to clipboard is only implemented on macOS".to_string())
    }
}

/// ⌘C outside single-photo mode (grid selection, or dev mode before a
/// develop has produced a preview) — same source priority as the
/// `reveal://thumb` protocol handler (developed sidecar → embedded camera
/// preview → companion JPEG), but full quality, not the grid's downscaled
/// copy. Native NSPasteboard write for the same reason as
/// `copy_developed_preview_to_clipboard`: `fetch()` on `reveal://thumb`
/// itself throws "TypeError: Load failed" in this WebView (reproduced
/// 2026-08-02) — a pre-existing bug in the browser-side clipboard path, not
/// specific to blob: URLs.
#[tauri::command]
pub(crate) async fn copy_photo_preview_to_clipboard(path: String) -> Result<(), String> {
    if apple_photos::is_asset(&path) {
        let jpeg = tauri::async_runtime::spawn_blocking(move || apple_photos::thumbnail(&path, 2560))
            .await.map_err(|e| e.to_string())??;
        #[cfg(target_os = "macos")]
        return macos::clipboard::write_jpeg_image(&jpeg);
        #[cfg(not(target_os = "macos"))]
        { let _ = jpeg; return Err("Apple Photos requires macOS".to_string()); }
    }
    let source = std::path::Path::new(&path);
    let jpeg = preview_sidecar_path(source)
        .filter(|candidate| candidate.is_file())
        .and_then(|candidate| std::fs::read(candidate).ok())
        .or_else(|| reveal_decode::extract_thumb_preview(source).ok().map(|p| p.bytes))
        .or_else(|| companion_jpeg_path(source).and_then(|candidate| std::fs::read(candidate).ok()))
        .ok_or_else(|| format!("No preview available for {path}"))?;
    #[cfg(target_os = "macos")]
    {
        return macos::clipboard::write_jpeg_image(&jpeg);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = jpeg;
        Err("copy to clipboard is only implemented on macOS".to_string())
    }
}

/// Publishing happens off the render path, so two settles in quick
/// succession can be in flight at once. Whichever STARTED last must be the
/// one that lands, or the sidecar ends up holding an older recipe than the
/// canvas — the exact stale-preview bug this file has already had once.
pub(crate) static PUBLISH_GENERATION: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, u64>>,
> = std::sync::OnceLock::new();

/// Claim the right to publish this photo, invalidating any in-flight publish.
pub(crate) fn next_publish_generation(path: &str) -> u64 {
    let map = PUBLISH_GENERATION.get_or_init(Default::default);
    let mut guard = map.lock().unwrap_or_else(|e| e.into_inner());
    let slot = guard.entry(path.to_string()).or_insert(0);
    *slot += 1;
    *slot
}

/// Is this publish still the newest one claimed for the photo?
pub(crate) fn publish_generation_is_current(path: &str, generation: u64) -> bool {
    let map = PUBLISH_GENERATION.get_or_init(Default::default);
    let guard = map.lock().unwrap_or_else(|e| e.into_inner());
    guard.get(path).is_none_or(|current| *current == generation)
}

/// THE CONTRACT: every preview-serving path, for every develop engine, MUST
/// funnel its final JPEG bytes through this one function on settle (durable).
/// This is the only place that PUBLISHES a render, and it publishes to both
/// stores at once: the `.preview.jpg` sidecar — the single source of truth
/// the grid, external tools and file-over-app all read — and the local cache
/// that spares the next visit a trip to the NAS.
///
/// Both, from the same bytes, in one call. The local cache was added without
/// extending this contract, so five places wrote it and each had to decide
/// for itself whether what it held was still current; one of them forgot,
/// and `.preview.jpg` started receiving the render from one slider ago.
/// Publishing is one act, so it is one function.
///
/// An engine whose interactive path doesn't produce JPEG directly (e.g. a
/// fast RGBA/canvas proxy) must still call this — see `write_preview_sidecar`
/// below, which renders a JPEG via the SAME engine-agnostic `develop_jpeg`
/// just for this purpose. Skipping this funnel is exactly the bug that let
/// Rapid-developed photos never touch disk while their loupe still updated.
///
/// Returns the published version — the sidecar's mtime in ms — so callers can
/// hand the frontend a token that agrees with what is on disk. It used to send
/// `Date.now()` instead, which busts the webview's own cache fine but can
/// never match a file, so nothing on disk could be addressed by it.
pub(crate) fn write_preview_sidecar_bytes(
    app: &tauri::AppHandle,
    path: &str,
    jpeg: &[u8],
    durable: bool,
    max_px: u32,
) -> u64 {
    if !durable {
        return 0;
    }
    let source = std::path::Path::new(path);
    let Some(sidecar) = preview_sidecar_path(source) else {
        return 0;
    };
    if let Err(e) = write_sidecar_if_changed(&sidecar, jpeg) {
        eprintln!("preview sidecar write {path}: {e}");
    }
    let version = served_preview_mtime(source);

    // The render at its own size, and — Francis's observation — the grid size
    // derived from the very same bytes rather than fetched or rendered again.
    // One downscale of what is already in hand spares the next grid visit a
    // NAS round trip AND the decode/re-encode it would repeat on every
    // request.
    let mut dests: Vec<std::path::PathBuf> = Vec::with_capacity(2);
    if let Ok(p) = developed_preview_cache_path(app, source, max_px, version) {
        dests.push(p);
    }
    let grid = (max_px > GRID_PREVIEW_EDGE).then(|| downscale_grid_thumb(jpeg.to_vec(), GRID_PREVIEW_EDGE, 1));
    if let (Some(bytes), Ok(p)) = (
        &grid,
        developed_preview_cache_path(app, source, GRID_PREVIEW_EDGE, version),
    ) {
        if let Err(e) = write_cache_entry(&p, bytes) {
            eprintln!("preview publish grid size {path}: {e}");
        }
    }
    for dest in &dests {
        if let Err(e) = write_cache_entry(dest, jpeg) {
            eprintln!("preview publish {path}: {e}");
        }
    }
    schedule_cache_prune(app);
    version
}

/// One cache entry, written so a half-written file can never be served: temp
/// file, then rename. Split out from the funnel so the write discipline is
/// testable without a running Tauri app.
pub(crate) fn write_cache_entry(dest: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    // An empty entry is worse than no entry: it is a cache HIT that serves
    // nothing, so the grid cell stays blank forever and never falls back to
    // the sidecar or the RAW. Nine of these existed on disk when Francis
    // reported scattered empty cells after a sort change (2026-09-23), all
    // keyed `-0` — photos with no sidecar at all.
    //
    // `downscale_grid_thumb` is how they get here: when the image crate
    // cannot decode its input it returns that input unchanged, so empty in
    // gives empty out, and the caller stores it as a success.
    if bytes.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "refusing to cache an empty preview",
        ));
    }
    let tmp = dest.with_extension("part");
    match std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, dest)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Pack a developed frame for the IPC bridge: four u32 of header, then the
/// RGBA bytes.
///
/// The obvious shape — a `#[derive(Serialize)]` struct with a `Vec<u8>` — is
/// what this replaces, and it was the reason dragging a slider felt heavy.
/// Serde sends a `Vec<u8>` as a JSON array of numbers, so one 2048px frame
/// (11.2 MB of pixels) became 44.7 MB of text: measured 68 ms to serialise in
/// release, before the webview had even begun parsing it, against ~24 ms to
/// actually render the frame on the GPU. We were paying ten times the render
/// just to cross the bridge, forty times a second.
///
/// `IpcResponse` hands the bytes over raw instead — the webview receives an
/// ArrayBuffer, and the pixels are read straight out of it with no parse and
/// no copy. `develop_preview` next door already did this for its JPEG; the
/// small payload had the fast path and the huge one did not.
pub(crate) fn pack_developed_frame(width: u32, height: u32, render_ms: u128, decode_ms: u128, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 + rgba.len());
    out.extend_from_slice(&width.to_le_bytes());
    out.extend_from_slice(&height.to_le_bytes());
    out.extend_from_slice(&(render_ms as u32).to_le_bytes());
    out.extend_from_slice(&(decode_ms as u32).to_le_bytes());
    out.extend_from_slice(rgba);
    out
}

#[tauri::command]
/// `live` is true while a slider is being dragged.
///
/// The frontend has always known this; the backend used to infer it from
/// `max_px < 2048`, which held only while live drags rendered a smaller
/// proxy. Once Rapid on the GPU became fast enough to drag at full 2048 that
/// inference silently became "every drag frame is a settled edit", and each
/// one re-rendered a JPEG, wrote `.preview.jpg` to the NAS and filled two
/// cache entries — mid-drag. Guesses about caller intent go stale; the caller
/// now says.
pub(crate) async fn develop_preview_rgba(
    app: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
    path: String,
    recipe: reveal_engine::Recipe,
    max_px: u32,
    live: Option<bool>,
) -> Result<IpcResponse, String> {
    let engine = state.0.clone();
    let out = {
        let engine = engine.clone();
        let path = path.clone();
        let recipe = recipe.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let source = apple_photos::source(&path)?;
            engine.develop_rgba8(&source, &recipe, max_px).map_err(|e| format!("{e:#}"))
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("{e:#}"))?
    };

    eprintln!(
        "develop_preview_rgba: {}x{} decode {} ms render {} ms",
        out.width, out.height, out.decode_ms, out.render_ms
    );

    // File over app: the canvas render above is display-only, in memory. The
    // grid (and every external tool) only ever sees the `.preview.jpg`
    // sidecar — without writing it here too, developing with Rapid never
    // touched disk, so the grid silently kept showing the camera JPEG no
    // matter which engine or look was picked.
    //
    // On settle only, and NOT awaited. A drag moves the photo on screen and
    // nothing else; letting go publishes, in the background. Awaiting it made
    // the canvas wait on a second JPEG render and a NAS write before showing
    // pixels it already had.
    if !live.unwrap_or(false) && max_px >= 2048 {
        let generation = next_publish_generation(&path);
        let app = app.clone();
        let engine = engine.clone();
        let path = path.clone();
        let recipe = recipe.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) =
                write_preview_sidecar(&app, &engine, &path, &recipe, max_px, generation).await
            {
                eprintln!("develop_preview_rgba sidecar write {path}: {e}");
            }
        });
    }

    Ok(IpcResponse::new(pack_developed_frame(
        out.width,
        out.height,
        out.render_ms,
        out.decode_ms,
        &out.rgba,
    )))
}

/// For engines whose interactive path is NOT JPEG (Rapid's canvas/RGBA proxy):
/// render one via the engine-agnostic `develop_jpeg` (dispatches through the
/// registry by `recipe.engine`, so this is correct for any engine) and pass it
/// through the one true funnel, `write_preview_sidecar_bytes`. This is the
/// pattern any FUTURE non-JPEG interactive engine must follow too.
pub(crate) async fn write_preview_sidecar(
    app: &tauri::AppHandle,
    engine: &std::sync::Arc<reveal_engine::Engine>,
    path: &str,
    recipe: &reveal_engine::Recipe,
    max_px: u32,
    generation: u64,
) -> Result<(), String> {
    // Render, then publish. This used to read the local cache first and reuse
    // whatever it found — which, once the cache key stopped carrying a recipe
    // digest, meant a settled Rapid edit republished the PREVIOUS render as
    // `.preview.jpg`: correct on the canvas, wrong in the grid. The cache
    // cannot answer "is this that recipe?", so it is not asked.
    let engine = engine.clone();
    let path_owned = path.to_string();
    let recipe_owned = recipe.clone();
    let rendered = tauri::async_runtime::spawn_blocking(move || {
        let source = apple_photos::source(&path_owned)?;
        engine.develop_jpeg(&source, &recipe_owned, max_px).map_err(|e| format!("{e:#}"))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| format!("{e:#}"))?;

    // The render above can take a second; another settle may have claimed the
    // photo meanwhile. Publishing now would put an older recipe on disk than
    // the one on screen.
    if !publish_generation_is_current(path, generation) {
        return Ok(());
    }
    write_preview_sidecar_bytes(app, path, &rendered.jpeg, true, max_px);
    Ok(())
}

/// A Google Takeout export hands back JPEGs still carrying a `.DNG`
/// extension — 113 in one folder here, 4032x3024, which libraw cannot touch,
/// so every RAW branch refuses them and the photo reads as "Preview
/// unavailable". Decoding is attempted, not assumed: a truly broken file
/// must still be reported as broken rather than served as bytes.
pub(crate) fn plain_image_bytes(path: &std::path::Path) -> Option<Vec<u8>> {
    let bytes = std::fs::read(path).ok()?;
    image::load_from_memory(&bytes).ok()?;
    Some(bytes)
}

#[cfg(test)]
mod preview_cache_tests {
    use super::prune_preview_cache;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    /// A scratch dir that cleans up after itself.
    struct Scratch(PathBuf);
    impl Scratch {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir()
                .join(format!("reveal-preview-cache-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// One cached render: `{photo}-2048-{variant}.jpg`, aged `secs` old. The
    /// mtime is set explicitly because every assertion here is about which
    /// file is the most recent.
    fn render(dir: &Path, photo: &str, variant: &str, secs: u64) -> PathBuf {
        let path = dir.join(format!("{photo}-2048-{variant}.jpg"));
        std::fs::write(&path, b"jpeg").unwrap();
        let when = SystemTime::now() - Duration::from_secs(secs);
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(when))
            .unwrap();
        path
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    /// The cache is for moving through photos, not for remembering old slider
    /// positions: only the current recipe can ever be asked for again.
    #[test]
    fn only_the_newest_render_of_a_photo_survives() {
        let s = Scratch::new("variants");
        render(&s.0, "aaaa", "old1", 300);
        render(&s.0, "aaaa", "old2", 200);
        render(&s.0, "aaaa", "current", 1);
        render(&s.0, "bbbb", "only", 50);

        prune_preview_cache(&s.0, 2000);

        assert_eq!(
            names(&s.0),
            vec!["aaaa-2048-current.jpg", "bbbb-2048-only.jpg"],
            "each photo keeps exactly its most recent render"
        );
    }

    /// The old code returned early whenever the library was under the limit,
    /// so on any normal cache — 99 photos against a limit of 2000 — nothing
    /// was ever cleaned and stale renders piled up indefinitely.
    #[test]
    fn stale_renders_are_cleaned_even_far_below_the_photo_limit() {
        let s = Scratch::new("below-limit");
        for i in 0..40 {
            render(&s.0, "aaaa", &format!("v{i}"), 1000 - i as u64);
        }
        prune_preview_cache(&s.0, 2000);
        assert_eq!(names(&s.0).len(), 1, "40 renders of one photo collapse to 1");
    }

    /// Publishing runs off the render path now, so two settles can be in
    /// flight at once. If the SLOWER, older one were allowed to land last,
    /// the sidecar would hold an older recipe than the canvas — the stale
    /// preview bug, back by a different door.
    ///
    /// Scope, stated because it is not obvious: this covers the MECHANISM,
    /// not its wiring. Deleting the check in `write_preview_sidecar` leaves
    /// this test green — the call site needs an AppHandle and an Engine, so
    /// it is not reachable from here. Treat a change to that `if` as
    /// untested.
    #[test]
    fn only_the_newest_claim_may_publish() {
        let path = format!("/nas/{}/DSCF0001.RAF", std::process::id());

        let first = super::next_publish_generation(&path);
        assert!(super::publish_generation_is_current(&path, first));

        // A second settle claims the photo while the first is still rendering.
        let second = super::next_publish_generation(&path);
        assert!(second > first, "each claim supersedes the last");
        assert!(
            !super::publish_generation_is_current(&path, first),
            "the older render must not publish"
        );
        assert!(super::publish_generation_is_current(&path, second));
    }

    /// Claims are per photo — developing one must not silence another.
    #[test]
    fn a_claim_on_one_photo_leaves_others_alone() {
        let a = format!("/nas/{}/A.RAF", std::process::id());
        let b = format!("/nas/{}/B.RAF", std::process::id());
        let claim_a = super::next_publish_generation(&a);
        super::next_publish_generation(&b);
        super::next_publish_generation(&b);
        assert!(
            super::publish_generation_is_current(&a, claim_a),
            "B's edits must not invalidate A's pending publish"
        );
    }

    /// Offline, the newest local render of a photo is the answer — there is
    /// no fresher truth to compare it against. Picking the newest matters:
    /// several versions sit side by side until the prune runs.
    #[test]
    fn the_offline_fallback_picks_the_newest_version_on_disk() {
        let s = Scratch::new("offline-newest");
        let older = render(&s.0, "aaaa", "", 900); // aaaa-2048-.jpg
        let newer = s.0.join("aaaa-2048-9a.jpg");
        std::fs::write(&newer, b"newer").unwrap();

        assert_eq!(
            super::newest_matching(&s.0, "aaaa-2048-").as_deref(),
            Some(newer.as_path())
        );
        assert!(older.exists(), "the older one was there to be chosen wrongly");
    }

    /// The prefix carries both the photo and the size. Getting either wrong
    /// serves one photo's pixels for another, or a 768 for a 2048 request.
    #[test]
    fn the_offline_fallback_never_crosses_photos_or_sizes() {
        let s = Scratch::new("offline-prefix");
        std::fs::write(s.0.join("aaaa-768-1f.jpg"), b"a-grid").unwrap();
        std::fs::write(s.0.join("aaaa-2048-1f.jpg"), b"a-full").unwrap();
        std::fs::write(s.0.join("bbbb-2048-1f.jpg"), b"b-full").unwrap();

        let read = |prefix: &str| {
            super::newest_matching(&s.0, prefix).map(|p| std::fs::read(p).unwrap())
        };
        assert_eq!(read("aaaa-2048-").as_deref(), Some(&b"a-full"[..]));
        assert_eq!(read("aaaa-768-").as_deref(), Some(&b"a-grid"[..]));
        assert_eq!(read("bbbb-2048-").as_deref(), Some(&b"b-full"[..]));
        assert_eq!(super::newest_matching(&s.0, "cccc-2048-"), None);
    }

    /// A `.DNG` that is really a JPEG must still display. The extension is
    /// not evidence; only a successful decode is.
    #[test]
    fn a_jpeg_wearing_a_raw_extension_is_still_served() {
        let s = Scratch::new("mislabelled");

        // A real 2x2 JPEG, encoded here rather than hand-written, so the
        // decode being attempted is a decode of something genuine.
        let img = image::RgbImage::from_fn(2, 2, |x, y| {
            image::Rgb([(x * 100) as u8, (y * 100) as u8, 40])
        });
        let mut jpeg = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut jpeg, image::ImageFormat::Jpeg)
            .unwrap();
        let jpeg = jpeg.into_inner();

        let masquerading = s.0.join("IMG_6163.DNG");
        std::fs::write(&masquerading, &jpeg).unwrap();
        assert_eq!(
            super::plain_image_bytes(&masquerading).as_deref(),
            Some(&jpeg[..]),
            "a JPEG named .DNG is served"
        );

        // Genuinely broken stays broken — this must not become "serve
        // anything that happens to be on disk".
        let junk = s.0.join("IMG_9999.DNG");
        std::fs::write(&junk, b"not an image at all").unwrap();
        assert!(super::plain_image_bytes(&junk).is_none(), "junk is still refused");
        assert!(super::plain_image_bytes(&s.0.join("absent.DNG")).is_none());
    }

    /// A cache entry must never appear half-written — a truncated JPEG in
    /// the cache would be SERVED, since the cache is consulted first.
    #[test]
    fn a_cache_entry_is_written_whole_or_not_at_all() {
        let s = Scratch::new("entry-atomic");
        let dest = s.0.join("aaaa-768-1f.jpg");
        super::write_cache_entry(&dest, b"render-A").unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"render-A");

        super::write_cache_entry(&dest, b"render-B").unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"render-B", "a republish replaces it");
        assert!(!dest.with_extension("part").exists(), "no leftover temp file");
    }

    /// The name is built in one place and taken apart in another. If those
    /// two ever disagree, the prune groups wrongly and silently deletes the
    /// wrong entries — so they are checked against each other here.
    #[test]
    fn cache_names_round_trip_through_the_prune_grouping() {
        let photo = Path::new("/nas/2026/DSCF0001.RAF");
        let grid = super::developed_preview_cache_name(photo, 768, 0x1f2e);
        let full = super::developed_preview_cache_name(photo, 2048, 0x1f2e);
        let newer = super::developed_preview_cache_name(photo, 768, 0x9a9a);

        let group = |name: &str| {
            let mut parts = name.splitn(3, '-');
            format!("{}-{}", parts.next().unwrap(), parts.next().unwrap())
        };
        assert_ne!(group(&grid), group(&full), "two sizes are two groups");
        assert_eq!(
            group(&grid),
            group(&newer),
            "two versions of one size are one group, so the older is pruned"
        );
    }

    /// The whole point of the size in the key: the grid's entry and the
    /// viewer's entry for the same photo must survive together.
    #[test]
    fn two_sizes_of_one_photo_do_not_evict_each_other() {
        let s = Scratch::new("two-sizes");
        std::fs::write(s.0.join("aaaa-768-1f.jpg"), b"grid").unwrap();
        std::fs::write(s.0.join("aaaa-2048-1f.jpg"), b"full").unwrap();

        prune_preview_cache(&s.0, 10_000);

        assert_eq!(names(&s.0).len(), 2, "both sizes survive");
    }

    /// An external edit to `.preview.jpg` moves the version, and the stale
    /// entry must not linger beside the new one.
    /// An empty entry is a cache hit that serves nothing, so the grid cell
    /// stays blank for good and never falls back to the sidecar or the RAW.
    /// Nine were on disk when scattered empty cells were reported;
    /// `downscale_grid_thumb` makes them, because the image crate returns its
    /// input unchanged when it cannot decode it, so empty in is empty out.
    #[test]
    fn an_empty_preview_is_never_cached() {
        let dir = std::env::temp_dir().join(format!("reveal-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let dest = dir.join("abc-768-0.jpg");

        assert!(super::write_cache_entry(&dest, b"").is_err());
        assert!(!dest.exists(), "nothing on disk, not even an empty file");
        assert!(
            !dest.with_extension("part").exists(),
            "and no half-written temp left behind either"
        );

        super::write_cache_entry(&dest, b"real-bytes").unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"real-bytes");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_new_version_replaces_the_old_one_for_that_size() {
        let s = Scratch::new("versions");
        let old = s.0.join("aaaa-768-1f.jpg");
        let new = s.0.join("aaaa-768-9a.jpg");
        std::fs::write(&old, b"before").unwrap();
        std::fs::write(&new, b"after").unwrap();
        let when = std::time::SystemTime::now() - std::time::Duration::from_secs(600);
        std::fs::File::options().write(true).open(&old).unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(when)).unwrap();

        prune_preview_cache(&s.0, 10_000);

        assert_eq!(names(&s.0), vec!["aaaa-768-9a.jpg"], "only the current version stays");
    }

    /// The budget evicts least-recently-touched first. Getting the ORDER
    /// wrong is the only way to do real harm here — evicting costs one NAS
    /// read, evicting the WRONG thing costs it on the photo you are using.
    #[test]
    fn the_budget_evicts_the_least_recently_touched_first() {
        let s = Scratch::new("budget");
        // 100 bytes each, oldest to newest.
        for (photo, age) in [("old", 900u64), ("mid", 600), ("new", 10)] {
            let path = s.0.join(format!("{photo}-768-1f.jpg"));
            std::fs::write(&path, vec![0u8; 100]).unwrap();
            let when = std::time::SystemTime::now() - std::time::Duration::from_secs(age);
            std::fs::File::options().write(true).open(&path).unwrap()
                .set_times(std::fs::FileTimes::new().set_modified(when)).unwrap();
        }

        prune_preview_cache(&s.0, 250); // room for two

        assert_eq!(
            names(&s.0),
            vec!["mid-768-1f.jpg", "new-768-1f.jpg"],
            "the oldest goes, the two most recent stay"
        );
    }

    /// A budget that fits everything must not evict anything.
    #[test]
    fn a_budget_with_room_to_spare_evicts_nothing() {
        let s = Scratch::new("budget-roomy");
        for photo in ["a", "b", "c"] {
            std::fs::write(s.0.join(format!("{photo}-768-1f.jpg")), vec![0u8; 100]).unwrap();
        }
        prune_preview_cache(&s.0, 10_000);
        assert_eq!(names(&s.0).len(), 3);
    }

    #[test]
    fn an_empty_or_missing_directory_is_not_an_error() {
        let s = Scratch::new("empty");
        prune_preview_cache(&s.0, 2000);
        assert!(names(&s.0).is_empty());
        prune_preview_cache(&s.0.join("nope"), 2000);
    }
}

#[cfg(test)]
mod orientation_tests {
    use super::downscale_grid_thumb;

    /// A landscape JPEG with no EXIF at all — what an iPhone DNG's embedded
    /// preview looks like once libraw hands it over.
    fn silent_landscape() -> Vec<u8> {
        let img = image::DynamicImage::new_rgb8(40, 20);
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Jpeg).unwrap();
        out.into_inner()
    }

    fn dims(bytes: &[u8]) -> (u32, u32) {
        let img = image::load_from_memory(bytes).unwrap();
        (img.width(), img.height())
    }

    /// The bug of 2026-09-23: the DNG said "rotate 90°", its preview said
    /// nothing, and the portrait was served lying on its side.
    #[test]
    fn a_silent_preview_takes_the_source_orientation() {
        assert_eq!(dims(&downscale_grid_thumb(silent_landscape(), 768, 6)), (20, 40));
        assert_eq!(dims(&downscale_grid_thumb(silent_landscape(), 768, 8)), (20, 40));
    }

    #[test]
    fn an_upright_source_leaves_it_alone() {
        assert_eq!(dims(&downscale_grid_thumb(silent_landscape(), 768, 1)), (40, 20));
        assert_eq!(dims(&downscale_grid_thumb(silent_landscape(), 768, 3)), (40, 20));
    }
}
