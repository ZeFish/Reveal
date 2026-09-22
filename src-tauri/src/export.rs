//! Exporting developed photos: to a folder, to a daily note, or in batch.
//!
//! Lifted out of `lib.rs` unchanged — see that file's header.

use crate::publishing::{vault_attachment_dir, vault_path};
use crate::*;

/// Export one photo (its saved recipe unless one is passed) to `dest_dir`.
#[tauri::command]
pub(crate) async fn export_photo(
    state: tauri::State<'_, EngineState>,
    path: String,
    recipe: reveal_engine::Recipe,
    dest_dir: String,
    long_edge: u32,
    border_frac: f32,
) -> Result<String, String> {
    let engine = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let src = std::path::Path::new(&path);
        let (jpeg, w, h) = engine
            .export_jpeg(&apple_photos::source(&path)?, &recipe, long_edge, border_frac)
            .map_err(|e| format!("{e:#}"))?;
        // Empty destination = the Swift default: the Desktop.
        let dest_dir = if dest_dir.is_empty() {
            let home = std::env::var("HOME").map_err(|e| e.to_string())?;
            format!("{home}/Desktop")
        } else {
            dest_dir
        };
        let stem = src.file_stem().unwrap_or_default().to_string_lossy();
        let out = std::path::Path::new(&dest_dir).join(format!("{stem}.jpg"));
        let out = write_photo_export(&path, &out, &jpeg)?;
        eprintln!("export: {} ({}x{})", out.display(), w, h);
        Ok(out.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Export one photo to the Obsidian vault attachment directory and append it to the capture-date Daily Note.
#[tauri::command]
pub(crate) async fn export_to_daily_note(
    app: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
    path: String,
    recipe: Option<reveal_engine::Recipe>,
    long_edge: u32,
    border_frac: f32,
) -> Result<String, String> {
    let engine = state.0.clone();
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let prefs = load_preferences(app_handle.clone());
        let obsidian_enabled = prefs
            .get("obsidian_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !obsidian_enabled {
            return Err("Obsidian integration is disabled in Settings.".to_string());
        }

        let metadata = apple_photos::metadata_path(&path)?;
        let source = apple_photos::source(&path)?;
        let src = std::path::Path::new(&path);
        let stem = src.file_stem().unwrap_or_default().to_string_lossy();
        let attachment_filename = format!("{stem}.jpg");

        let dest_dir = vault_attachment_dir(app_handle.clone())?;
        let out = std::path::Path::new(&dest_dir).join(&attachment_filename);

        let final_recipe = match recipe {
            Some(r) => r,
            None => reveal_meta::read(&metadata)
                .map_err(|e| e.to_string())?
                .and_then(|s| s.engine_settings)
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default(),
        };

        let (jpeg, _, _) = engine
            .export_jpeg(&source, &final_recipe, long_edge, border_frac)
            .map_err(|e| format!("{e:#}"))?;
        let out = write_photo_export(&path, &out, &jpeg)?;
        let attachment_filename = out.file_name().ok_or("Export filename is missing")?.to_string_lossy().into_owned();

        let capture_dt: chrono::DateTime<chrono::Local> = if let Some(ts) = photo_capture_timestamp(&path)? {
            chrono::DateTime::from_timestamp(ts, 0)
                .map(|utc| utc.with_timezone(&chrono::Local))
                .unwrap_or_else(chrono::Local::now)
        } else if let Ok(meta) = std::fs::metadata(src) {
            if let Ok(mtime) = meta.modified() {
                chrono::DateTime::from(mtime)
            } else {
                chrono::Local::now()
            }
        } else {
            chrono::Local::now()
        };

        let caption = reveal_meta::read(&metadata)
            .map_err(|e| e.to_string())?
            .and_then(|s| s.description);

        let vault = vault_path(&app_handle);
        let prefs = load_preferences(app_handle);
        let logs_folder = prefs.get("logs_folder").and_then(|v| v.as_str()).map(str::to_string);
        let daily = daily_note::DailyNote::new(vault, logs_folder);
        let note_path = daily.append_photos(&[attachment_filename], caption.as_deref(), capture_dt)?;

        Ok(note_path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Batch-export photos to the Obsidian vault attachment directory and append them to the Daily Note.
#[tauri::command]
pub(crate) async fn export_batch_to_daily_note(
    app: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
    paths: Vec<String>,
    long_edge: u32,
    border_frac: f32,
) -> Result<String, String> {
    let engine = state.0.clone();
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let prefs = load_preferences(app_handle.clone());
        let obsidian_enabled = prefs
            .get("obsidian_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !obsidian_enabled {
            return Err("Obsidian integration is disabled in Settings.".to_string());
        }

        let dest_dir = vault_attachment_dir(app_handle.clone())?;
        let vault = vault_path(&app_handle);
        let logs_folder = prefs.get("logs_folder").and_then(|v| v.as_str()).map(str::to_string);
        let daily = daily_note::DailyNote::new(vault, logs_folder);

        let mut count = 0usize;
        let mut last_note = String::new();

        for path in &paths {
            let metadata = apple_photos::metadata_path(path)?;
            let source = apple_photos::source(path)?;
            let src = std::path::Path::new(path);
            let stem = src.file_stem().unwrap_or_default().to_string_lossy();
            let attachment_filename = format!("{stem}.jpg");
            let out = std::path::Path::new(&dest_dir).join(&attachment_filename);

            let recipe = reveal_meta::read(&metadata)
                .map_err(|e| e.to_string())?
                .and_then(|s| s.engine_settings)
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();

            let (jpeg, _, _) = engine
                .export_jpeg(&source, &recipe, long_edge, border_frac)
                .map_err(|e| format!("Could not export {}: {e:#}", src.display()))?;
            let out = write_photo_export(path, &out, &jpeg)?;
            let attachment_filename = out.file_name()
                .ok_or("Export filename is missing")?.to_string_lossy().into_owned();
            let capture_dt: chrono::DateTime<chrono::Local> = if let Some(ts) = photo_capture_timestamp(path)? {
                chrono::DateTime::from_timestamp(ts, 0)
                    .map(|utc| utc.with_timezone(&chrono::Local))
                    .unwrap_or_else(chrono::Local::now)
            } else if let Ok(meta) = std::fs::metadata(src) {
                if let Ok(mtime) = meta.modified() {
                    chrono::DateTime::from(mtime)
                } else {
                    chrono::Local::now()
                }
            } else {
                chrono::Local::now()
            };

            let caption = reveal_meta::read(&metadata)
                .map_err(|e| e.to_string())?
                .and_then(|s| s.description);

            let np = daily.append_photos(&[attachment_filename], caption.as_deref(), capture_dt)?;
            last_note = np.to_string_lossy().into_owned();
            count += 1;
        }

        if count == 0 {
            return Err("No photos could be exported to daily note".to_string());
        }

        Ok(last_note)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn photo_capture_timestamp(path: &str) -> Result<Option<i64>, String> {
    if apple_photos::is_asset(path) {
        Ok(apple_photos::info(path)?.capture_at)
    } else {
        Ok(reveal_decode::capture_timestamp(std::path::Path::new(path)))
    }
}

/// Photos can contain many distinct assets named IMG_0001. Preserve the name
/// when available, otherwise number the export instead of overwriting another.
pub(crate) fn write_photo_export(
    path: &str,
    requested: &std::path::Path,
    jpeg: &[u8],
) -> Result<std::path::PathBuf, String> {
    use std::io::Write;
    let parent = requested.parent().ok_or("Export destination has no parent")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    if !apple_photos::is_asset(path) {
        std::fs::write(requested, jpeg).map_err(|e| e.to_string())?;
        return Ok(requested.to_path_buf());
    }
    let stem = requested.file_stem().ok_or("Export filename is missing")?.to_string_lossy();
    for number in 0..10_000 {
        let out = if number == 0 {
            requested.to_path_buf()
        } else {
            parent.join(format!("{stem}-{number}.jpg"))
        };
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&out) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(jpeg) {
                    if let Err(cleanup) = std::fs::remove_file(&out) {
                        eprintln!("Could not remove incomplete export {}: {cleanup}", out.display());
                    }
                    return Err(error.to_string());
                }
                return Ok(out);
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    }
    Err("Too many exports with this name; choose another destination".to_string())
}

/// Develop and export a batch using each photo's saved recipe and one progress stream.
pub(crate) fn export_batch(
    engine: &reveal_engine::Engine,
    app: &tauri::AppHandle,
    cancelled: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    paths: &[String],
    dest_dir: &str,
    long_edge: u32,
    border_frac: f32,
    event_name: &str,
) -> Result<usize, String> {
    let total = paths.len();
    let mut done = 0usize;
    std::fs::create_dir_all(dest_dir).map_err(|e| e.to_string())?;
    for (i, path) in paths.iter().enumerate() {
        if cancelled.load(std::sync::atomic::Ordering::Acquire) {
            let _ = app.emit(
                event_name,
                serde_json::json!({
                    "done": done,
                    "total": total,
                    "current": "",
                    "phase": "cancelled",
                    "cancelled": true
                }),
            );
            return Ok(done);
        }
        let src = std::path::Path::new(path);
        let name = src.file_name().unwrap_or_default().to_string_lossy();
        let _ = app.emit(
            event_name,
            serde_json::json!({ "done": i, "total": total, "current": name }),
        );
        let recipe = reveal_meta::read(&apple_photos::metadata_path(path)?)
            .map_err(|e| e.to_string())?
            .and_then(|s| s.engine_settings)
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let source = apple_photos::source(path)?;
        let (jpeg, _, _) = engine
            .export_jpeg(&source, &recipe, long_edge, border_frac)
            .map_err(|e| format!("Could not export {name}: {e:#}"))?;
        let stem = src.file_stem().unwrap_or_default().to_string_lossy();
        let out = std::path::Path::new(dest_dir).join(format!("{stem}.jpg"));
        write_photo_export(path, &out, &jpeg)?;
        done += 1;
    }
    let _ = app.emit(
        event_name,
        serde_json::json!({ "done": total, "total": total, "current": "" }),
    );
    Ok(done)
}

/// Batch-export every frame of `paths` using each photo's SAVED recipe
/// (engine defaults when none). Emits `export-progress`.
#[tauri::command]
pub(crate) async fn export_photos(
    app: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
    cancellation: tauri::State<'_, ExportState>,
    paths: Vec<String>,
    dest_dir: String,
    long_edge: u32,
    border_frac: f32,
) -> Result<usize, String> {
    let engine = state.0.clone();
    let cancelled = cancellation.0.clone();
    cancelled.store(false, std::sync::atomic::Ordering::Release);
    tauri::async_runtime::spawn_blocking(move || {
        // Empty destination = the Swift default: the Desktop. A configured
        // export folder arrives as an absolute path from the panel.
        let dest_dir = if dest_dir.is_empty() {
            let home = std::env::var("HOME").map_err(|e| e.to_string())?;
            format!("{home}/Desktop")
        } else {
            dest_dir
        };
        let total = paths.len();
        let done = export_batch(&engine, &app, &cancelled, &paths, &dest_dir, long_edge, border_frac, "export-progress")?;
        eprintln!("export batch: {done}/{total} → {dest_dir}");
        Ok(done)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) fn cancel_exports(cancellation: tauri::State<'_, ExportState>) {
    cancellation
        .0
        .store(true, std::sync::atomic::Ordering::Release);
}
