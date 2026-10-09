//! Photo development engine orchestration, presets, sidecars, and thumbnails priority.

use rayon::prelude::*;
use tauri::Manager;
use crate::blocking;
use crate::preview::*;

pub struct IndexState(pub std::sync::Arc<reveal_index::Index>);

/// Lets the importer remember, in the catalogue, what it has already hashed.
pub struct CatalogueHashes(pub std::sync::Arc<reveal_index::Index>);

impl reveal_import::HashCache for CatalogueHashes {
    fn get(&self, path: &std::path::Path) -> Option<String> {
        self.0.content_hash(&path.to_string_lossy())
    }
    fn put(&self, path: &std::path::Path, hash: &str) {
        if let Err(e) = self.0.set_content_hash(&path.to_string_lossy(), hash) {
            eprintln!("import: could not cache hash for {}: {e}", path.display());
        }
    }
}

pub struct ExportState(pub std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Default for ExportState {
    fn default() -> Self {
        Self(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)))
    }
}

/// One AI cull runs at a time per day-folder — mirrors `ImportState`'s
/// in-flight guard so triggering it twice for the same folder is a clear error.
#[derive(Clone, Default)]
pub struct CullState(pub std::sync::Arc<std::sync::Mutex<std::collections::BTreeSet<String>>>);

pub struct CullCancelState(pub std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Default for CullCancelState {
    fn default() -> Self {
        Self(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)))
    }
}

pub struct EngineState(pub std::sync::Arc<reveal_engine::Engine>);

/// spektrafilm data dir: bundled resource in the .app, the repo copy in dev.
pub fn resolve_data_dir(app: &tauri::App) -> std::path::PathBuf {
    if let Ok(dir) = app.path().resource_dir() {
        let candidate = dir.join("data");
        if candidate.join("profiles").is_dir() {
            return candidate;
        }
    }
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// The engine's default recipe — single source of truth for the panel.
#[tauri::command]
pub fn default_recipe() -> reveal_engine::Recipe {
    reveal_engine::Recipe::default()
}

/// The dev panel's INFO spec sheet — EXIF straight off the RAW's metadata
/// block (no pixel decode, so it's cheap even over the NFS mount).
#[derive(serde::Serialize)]
pub struct ExifInfo {
    pub aperture: Option<f32>,
    pub shutter: Option<String>,
    pub iso: Option<u32>,
    pub focal_mm: Option<f32>,
    pub captured_at: Option<String>,
    pub make: String,
    pub model: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[tauri::command]
pub async fn frame_info(path: String) -> Result<ExifInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if crate::apple_photos::is_asset(&path) {
            let asset = crate::apple_photos::info(&path)?;
            return Ok(ExifInfo {
                aperture: None, shutter: None, iso: None, focal_mm: None,
                captured_at: asset.capture_at.and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
                    .map(|date| date.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string()),
                make: String::new(), model: String::new(),
                width: Some(asset.width), height: Some(asset.height),
            });
        }
        let src = rawler::rawsource::RawSource::new(std::path::Path::new(&path))
            .map_err(|e| format!("open: {e}"))?;
        let dec = rawler::get_decoder(&src).map_err(|e| format!("decoder: {e:?}"))?;
        let md = dec
            .raw_metadata(&src, &rawler::decoders::RawDecodeParams::default())
            .map_err(|e| format!("metadata: {e:?}"))?;
        let e = &md.exif;
        let dimensions = reveal_decode::capture_dimensions(std::path::Path::new(&path));
        let ratio = |r: &rawler::formats::tiff::Rational| r.n as f32 / r.d.max(1) as f32;
        let shutter = e.exposure_time.as_ref().map(|r| {
            if r.n >= r.d {
                format!("{:.0}s", ratio(r))
            } else {
                format!("1/{}", (r.d as f32 / r.n.max(1) as f32).round() as u32)
            }
        });
        // EXIF "2026:06:28 14:25:33" → "2026-06-28 · 14:25" (the Swift format).
        let captured_at = e
            .date_time_original
            .as_ref()
            .or(e.create_date.as_ref())
            .map(|s| {
                let s = s.replacen(':', "-", 2);
                match s.split_once(' ') {
                    Some((d, t)) => format!("{d} · {}", t.get(..5).unwrap_or(t)),
                    None => s,
                }
            });
        Ok(ExifInfo {
            aperture: e.fnumber.as_ref().map(ratio),
            shutter,
            iso: e.iso_speed_ratings.map(u32::from).or(e.iso_speed),
            focal_mm: e.focal_length.as_ref().map(ratio),
            captured_at,
            make: md.make,
            model: md.model,
            width: dimensions.map(|value| value.0),
            height: dimensions.map(|value| value.1),
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Engines registered in the darkroom engine registry.
#[tauri::command]
pub fn list_engines(
    state: tauri::State<'_, EngineState>,
) -> Vec<reveal_engine::EngineInfo> {
    state.0.list_engines()
}

/// Decode a photo into the engine's cache without rendering it, so stepping
/// to it is instant.
#[tauri::command]
pub async fn prefetch_photo(
    state: tauri::State<'_, EngineState>,
    thumbs: tauri::State<'_, crate::thumb_queue::ThumbQueueState>,
    path: String,
    neighbour: Option<bool>,
    warm: Option<bool>,
) -> Result<(), String> {
    let engine = state.0.clone();
    let thumbs = thumbs.0.clone();
    let neighbour = neighbour.unwrap_or(false);
    let warm = warm.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || {
        let _warming;
        let _wire = if warm {
            _warming = Some(thumbs.warming_after_visible(std::time::Duration::from_secs(15)));
            None
        } else if neighbour {
            match thumbs.foreground_after_current(std::time::Duration::from_secs(60)) {
                Some(wire) => Some(wire),
                None => {
                    eprintln!("[perf] prefetch_photo {path}: neighbour skipped, the person moved on");
                    return;
                }
            }
        } else {
            Some(thumbs.foreground(&path))
        };
        let t = std::time::Instant::now();
        eprintln!("[perf] prefetch_photo {path}: start ({})", if warm { "grid selection" } else if neighbour { "neighbour" } else { "the photo itself" });
        if let Ok(source) = crate::apple_photos::source(&path) {
            let source_ms = t.elapsed().as_millis();
            engine.prefetch(&source, 2048);
            eprintln!(
                "[perf] prefetch_photo {}: source {} ms, decoded+cached {} ms total",
                path,
                source_ms,
                t.elapsed().as_millis()
            );
        }
    })
    .await
    .map_err(|e| e.to_string())
}

/// The grid says which photos are on screen: their thumbnails are served first (see `thumb_queue`).
#[tauri::command]
pub fn set_visible_thumbs(state: tauri::State<'_, crate::thumb_queue::ThumbQueueState>, paths: Vec<String>) {
    state.0.set_visible(paths);
}

/// Develop is open (`true`) or closed (`false`): while it is, the grid's thumbnails decode one at
/// a time so the photo being worked on has the NAS link to itself (see `thumb_queue`).
#[tauri::command]
pub fn set_thumb_priority(state: tauri::State<'_, crate::thumb_queue::ThumbQueueState>, develop: bool) {
    state.0.set_background(develop);
}

/// Whether the Rapid engine's per-pixel pass can run on the GPU here.
#[tauri::command]
pub fn gpu_available() -> bool {
    reveal_engine::rapid_gpu::available()
}

/// Film and paper stocks available to the pickers.
#[tauri::command]
pub fn list_profiles(
    state: tauri::State<'_, EngineState>,
) -> Result<Vec<reveal_engine::ProfileEntry>, String> {
    state.0.list_profiles().map_err(|e| format!("{e:#}"))
}

/// User `.cube` LUTs available to the LUT-stack pickers (dev panel).
#[tauri::command]
pub fn list_luts(state: tauri::State<'_, EngineState>) -> Result<Vec<String>, String> {
    state.0.list_luts().map_err(|e| format!("{e:#}"))
}

pub fn presets_dir_for(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(crate::preset::presets_dir(
        &app.path().app_data_dir().map_err(|e| e.to_string())?,
    ))
}

#[tauri::command]
pub fn list_presets(app: tauri::AppHandle) -> Result<Vec<crate::preset::PresetEntry>, String> {
    Ok(crate::preset::list(&presets_dir_for(&app)?))
}

#[tauri::command]
pub fn save_preset(
    app: tauri::AppHandle,
    name: String,
    recipe: reveal_engine::Recipe,
) -> Result<(), String> {
    crate::preset::save(&presets_dir_for(&app)?, &name, &recipe)
}

#[tauri::command]
pub fn delete_preset(app: tauri::AppHandle, name: String) -> Result<(), String> {
    crate::preset::delete(&presets_dir_for(&app)?, &name)
}

/// Import Lightroom / Camera Raw `.xmp` presets as Reveal presets.
#[tauri::command]
pub async fn import_xmp_presets(app: tauri::AppHandle) -> Result<Vec<crate::xmp_preset::ImportReport>, String> {
    use tauri_plugin_dialog::DialogExt;
    let Some(files) = app
        .dialog()
        .file()
        .add_filter("Lightroom / Camera Raw preset", &["xmp"])
        .blocking_pick_files()
    else {
        return Ok(Vec::new()); // cancelled
    };

    let dir = presets_dir_for(&app)?;
    let mut reports = Vec::new();
    let mut errors = Vec::new();
    for file in files {
        let Ok(path) = file.into_path() else { continue };
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("{label}: {e}"));
                continue;
            }
        };
        match crate::xmp_preset::parse(&text) {
            Ok((recipe, report)) => {
                if let Err(e) = crate::preset::save(&dir, &report.name, &recipe) {
                    errors.push(format!("{label}: {e}"));
                } else {
                    reports.push(report);
                }
            }
            Err(e) => errors.push(format!("{label}: {e}")),
        }
    }

    if reports.is_empty() && !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    Ok(reports)
}

/// Read the photo's sidecar (rating, tags, saved recipe). Null when none.
#[tauri::command]
pub async fn load_sidecar(path: String) -> Result<Option<reveal_meta::Sidecar>, String> {
    blocking(move || crate::photo::Photo::new(path).sidecar()).await
}

/// Persist the recipe into the photo's sidecar, preserving the standard
/// fields (rating, caption, tags) already there.
pub fn write_recipe_to_sidecar(path: &std::path::Path, recipe: &reveal_engine::Recipe) -> Result<(), String> {
    crate::photo::Photo::new(path.to_string_lossy()).save_recipe(recipe)
}

#[tauri::command]
pub async fn save_recipe(path: String, recipe: reveal_engine::Recipe) -> Result<(), String> {
    blocking(move || write_recipe_to_sidecar(std::path::Path::new(&path), &recipe)).await
}

#[tauri::command]
pub async fn clear_recipe(path: String) -> Result<(), String> {
    let _ = next_publish_generation(&path);
    blocking(move || crate::photo::Photo::new(path).clear_development_now()).await
}

/// Owe the photo a clearing of its develop settings (the switch to "None"): written once it has
/// been left alone, or never if an engine is chosen again — see `photo_writes`.
#[tauri::command]
pub fn queue_clear_development(state: tauri::State<'_, crate::photo_writes::PhotoWritesState>, path: String) {
    state.0.queue_clear(&path);
}

/// Owe the photo this recipe: every edit calls it, the last one is written once the photo has
/// been left alone (and on the way out, if the app closes first) — see `photo_writes`.
#[tauri::command]
pub fn queue_save_recipe(
    state: tauri::State<'_, crate::photo_writes::PhotoWritesState>,
    path: String,
    recipe: reveal_engine::Recipe,
) {
    state.0.queue_save_recipe(&path, recipe);
}

/// Withdraw what was owed to the photo. `true` if something was waiting.
#[tauri::command]
pub fn cancel_photo_writes(state: tauri::State<'_, crate::photo_writes::PhotoWritesState>, path: String) -> bool {
    state.0.cancel(&path)
}

/// Per-frame develop-sidecar mtimes (ms since epoch, 0 = as-shot), parallel to
/// `paths`. The grid feeds these back as thumb cache-busting versions so an
/// external edit to a `.preview.jpg` shows up on the next load — file over app.
#[tauri::command]
pub async fn preview_versions(paths: Vec<String>) -> Result<Vec<u64>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        paths
            .par_iter()
            .map(|p| served_preview_mtime(std::path::Path::new(p)))
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}
