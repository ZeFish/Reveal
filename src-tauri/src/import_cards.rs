//! Ingesting memory cards: finding them, copying their DCIM into the dated archive, ejecting them.
//!
//! Lifted out of `lib.rs` unchanged — see that file's header.

use crate::catalog::is_writable_dir;
use crate::*;

/// Cards (removable volumes with a DCIM of RAWs) currently mounted.
#[tauri::command]
pub(crate) async fn find_cards() -> Vec<reveal_import::Card> {
    reveal_import::find_cards()
}

/// Ingest a card's DCIM into the archive's dated layout. Emits
/// `import-progress` {done, total, current} along the way.
#[tauri::command]
pub(crate) async fn import_card(
    app: tauri::AppHandle,
    index_state: tauri::State<'_, IndexState>,
    import_state: tauri::State<'_, ImportState>,
    cancel_state: tauri::State<'_, ImportCancelState>,
    dcim: String,
    archive: String,
) -> Result<reveal_import::ImportStats, String> {
    // The Settings panel has always offered a date-folder pattern; until now
    // it stopped at the preferences file and the import crate used its own
    // hardcoded shape, so the setting looked live and did nothing.
    let date_format = load_preferences(app.clone())
        .get("date_folders")
        .and_then(|v| v.as_str())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(reveal_import::DEFAULT_DATE_FORMAT)
        .to_string();

    {
        let mut running = import_state.0.lock().unwrap();
        if running.contains(&dcim) {
            return Err("import already in progress for this card".into());
        }
        running.insert(dcim.clone());
    }
    // Reachability guard — catches the common "primary root is a NAS that
    // isn't mounted right now" case before we spin up a worker that would
    // fail every copy. We require the archive to be an existing directory
    // we can write into. A missing/stale mount path returns a clear error
    // the HUD surfaces (via import-failed) instead of churning silently.
    let archive_path = std::path::Path::new(&archive);
    if !is_writable_dir(archive_path) {
        let msg = format!(
            "Destination not found: \u{201c}{}\u{201d}. Mount the volume or choose an import folder.",
            archive
        );
        import_state.0.lock().unwrap().remove(&dcim);
        let _ = app.emit(
            "import-failed",
            serde_json::json!({ "dcim": dcim, "message": msg }),
        );
        show_import_panel(&app);
        return Err(msg);
    }
    // Fresh stop flag for this run (the HUD's stop button flips it).
    cancel_state.0.store(false, std::sync::atomic::Ordering::Relaxed);

    let _ = app.emit(
        "import-started",
        serde_json::json!({ "dcim": dcim, "archive": archive }),
    );
    show_import_panel(&app);

    // Resolve the configured default-import preset (if any) to its Recipe
    // now, up front — a name that no longer matches a saved preset (deleted
    // since it was set) just means no recipe gets applied, same as having
    // none configured, rather than failing the whole import.
    let default_import_recipe: Option<reveal_engine::Recipe> = read_shell_prefs(&app)
        .default_import_preset
        .and_then(|name| {
            let dir = presets_dir_for(&app).ok()?;
            preset::list(&dir).into_iter().find(|p| p.name == name).map(|p| p.recipe)
        });

    let state = import_state.0.clone();
    let cancel = cancel_state.0.clone();
    let dcim_key = dcim.clone();
    let app_for_worker = app.clone();
    let idx = index_state.0.clone();
    let worker = tauri::async_runtime::spawn_blocking(move || {
        let sources = reveal_import::collect_raws(std::path::Path::new(&dcim));
        let mut report = |done: usize, total: usize, current: &str, path: &str, dest_path: &str, dest_dir: &str| {
            // Apply the default preset the moment a photo lands, before the
            // grid/index even has a chance to show it — so it never has a
            // visible "as-shot" flash before developing itself.
            if !dest_path.is_empty() {
                if let Some(recipe) = &default_import_recipe {
                    if let Err(e) = write_recipe_to_sidecar(std::path::Path::new(dest_path), recipe) {
                        eprintln!("import: échec de l'application du preset par défaut à {dest_path} : {e}");
                    }
                }
                // Seed the local cache from the card, which is mounted and
                // fast right now, so the frames you just shot browse at local
                // speed instead of each one costing a first NAS round trip
                // (Francis: "les dernières photos importées doivent être
                // cachées"). The camera's own preview is the right source
                // here — nothing is developed yet.
                if let Ok(preview) = reveal_decode::extract_thumb_preview(std::path::Path::new(path)) {
                    // At grid size: this is the camera's own JPEG, seeding the
                    // surface that browses it. Version 0 — nothing has been
                    // developed yet, so there is no sidecar to have an mtime.
                    let sized = downscale_grid_thumb(preview.bytes, GRID_PREVIEW_EDGE);
                    cache_developed_preview_locally(
                        &app_for_worker,
                        std::path::Path::new(dest_path),
                        &sized,
                        GRID_PREVIEW_EDGE,
                        0,
                    );
                }
            }
            if !dest_dir.is_empty() {
                let _ = idx.scan_subtree_with(std::path::Path::new(dest_dir), |_, _| {});
            }
            let _ = app_for_worker.emit(
                "import-progress",
                serde_json::json!({
                    "done": done,
                    "total": total,
                    "current": current,
                    "path": path,
                    "dest": dest_path,
                    "destDir": dest_dir
                }),
            );
        };
        let stats = reveal_import::import(
            &sources,
            std::path::Path::new(&archive),
            &date_format,
            &CatalogueHashes(idx.clone()),
            &cancel,
            &mut report,
        )
        .map_err(|e| e.to_string())?;
        eprintln!(
            "import: {} copiés, {} skippés, {} échoués, {} Mo, {} ms{}",
            stats.copied,
            stats.skipped,
            stats.failed,
            stats.bytes / 1_048_576,
            stats.ms,
            if stats.cancelled { " (arrêté)" } else { "" }
        );
        Ok(stats)
    })
    .await;

    state.lock().unwrap().remove(&dcim_key);

    let result: Result<reveal_import::ImportStats, String> = match worker {
        Ok(result) => result,
        Err(e) => Err(e.to_string()),
    };

    match &result {
        Ok(stats) => {
            let _ = app.emit("import-finished", stats);
        }
        Err(message) => {
            let _ = app.emit(
                "import-failed",
                serde_json::json!({ "dcim": dcim_key, "message": message }),
            );
        }
    }

    result
}

/// Stop the running import between files — an in-flight copy finishes its
/// temp+rename, so the archive never sees a torn file (Swift `Importer.cancel`).
#[tauri::command]
pub(crate) fn cancel_import(cancel_state: tauri::State<'_, ImportCancelState>) {
    cancel_state.0.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Eject the card's volume after an import — the last step of the ingest
/// loop so the user can just pull the card out. `volume` is the mount point
/// (e.g. `/Volumes/X100F`); `diskutil eject` unmounts and powers it down.
#[tauri::command]
pub(crate) async fn eject_card(volume: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let status = std::process::Command::new("/usr/sbin/diskutil")
            .arg("eject")
            .arg(&volume)
            .status()
            .map_err(|e| format!("diskutil: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("eject failed ({volume})"))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
