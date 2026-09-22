//! AI-assisted culling: reading its preferences, scoring a folder, ranking
//! it with a vision model, and acting on the result.
//!
//! Lifted out of `lib.rs` unchanged. That file held all 96 Tauri commands in
//! 5313 lines, which made it the one place in the codebase where nothing
//! could be found reliably. The boundaries were already there and already
//! contiguous — they were just unnamed.

use crate::export::export_batch;
use crate::*;

/// The generic preferences bag (`prefs.json`, the same file
/// `load_preferences`/`save_preferences` read/write for the Settings modal)
/// — reused here rather than adding dedicated `ShellPrefs` fields + setter
/// commands for four AI-cull settings.
/// (mark_story, export_desktop, target, api_key, model) — the two outcomes
/// are independent toggles now, not one master switch: a walk-away run can
/// mark picks into the story, export JPEGs to the Desktop, or both.
pub(crate) fn read_ai_cull_prefs(
    app: &tauri::AppHandle,
) -> (bool, bool, u32, Option<String>, Option<String>, reveal_cull::AiProvider) {
    let default_provider = reveal_cull::AiProvider::Anthropic;
    let Ok(path) = prefs_file(app) else {
        return (false, false, 24, None, None, default_provider);
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return (false, false, 24, None, None, default_provider);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return (false, false, 24, None, None, default_provider);
    };
    let mark_story = value.get("ai_cull_mark_story").and_then(|v| v.as_bool()).unwrap_or(false);
    let export_desktop = value.get("ai_cull_export_desktop").and_then(|v| v.as_bool()).unwrap_or(false);
    let target = value
        .get("ai_cull_target")
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(24);
    let api_key = value
        .get("ai_api_key")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|s| !s.is_empty());
    let model = value
        .get("ai_model")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .filter(|s| !s.is_empty());
    let provider = value
        .get("ai_provider")
        .and_then(|v| v.as_str())
        .map(reveal_cull::AiProvider::from_str)
        .unwrap_or(default_provider);
    (mark_story, export_desktop, target, api_key, model, provider)
}

/// Local prefilter + cloud vision ranking, no side effects (no rating bump,
/// no export) — just the picks plus counts for whatever summary the caller
/// wants to show. Emits `cull-progress` (phase "local"/"cloud") on `app`
/// along the way. Shared by the walk-away `ai_cull` (which rates+exports
/// the result afterward) and the manual `ai_cull_selection` (whose caller
/// decides what to do with the picks — today, add them to the quick
/// collection).
fn score_paths(
    app: &tauri::AppHandle,
    dir: &str,
    paths: &[String],
    target: u32,
    api_key: &str,
    model: Option<String>,
    provider: reveal_cull::AiProvider,
    cancel: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<(Vec<String>, usize, usize), String> {
    let considered = paths.len();
    let _ = app.emit(
        "cull-progress",
        serde_json::json!({ "dir": dir, "phase": "local", "done": 0, "total": considered }),
    );
    let survivors = reveal_cull::prefilter(paths, &reveal_cull::PrefilterConfig::default());
    if cancel.load(std::sync::atomic::Ordering::Acquire) {
        return Err("cancelled".into());
    }
    let _ = app.emit(
        "cull-progress",
        serde_json::json!({ "dir": dir, "phase": "cloud", "done": 0, "total": survivors.len() }),
    );

    // Re-extract/re-encode only the survivors — keeping every candidate's
    // preview bytes in memory for a full day's import would be wasteful.
    let candidates: Vec<reveal_cull::RankCandidate> = survivors
        .iter()
        .filter_map(|s| {
            reveal_cull::embedded_preview_jpeg(&s.path, 768)
                .map(|jpeg_bytes| reveal_cull::RankCandidate { path: s.path.clone(), jpeg_bytes })
        })
        .collect();

    let model = model.unwrap_or_else(|| provider.default_model().to_string());
    let ranker = reveal_cull::vision_ranker(provider, api_key.to_string(), model);
    let candidate_count = candidates.len();
    let progress_ranker = ProgressRanker {
        inner: ranker.as_ref(),
        app,
        dir,
        total: candidate_count,
        done: std::sync::atomic::AtomicUsize::new(0),
    };
    let ranked = reveal_cull::rank_all(&progress_ranker, candidates, 20, target as usize)
        .map_err(|e| e.to_string())?;
    if cancel.load(std::sync::atomic::Ordering::Acquire) {
        return Err("cancelled".into());
    }

    let survivors_count = survivors.len();
    Ok((ranked.into_iter().map(|r| r.path).collect(), considered, survivors_count))
}

/// Wraps a `VisionRanker` to emit a `cull-progress` tick after every batch
/// completes, instead of once at the phase's start — the cloud stage is a
/// handful of sequential HTTP requests (one per ~20 photos) each taking real
/// seconds, so without this the UI freezes at "0/N" for the whole stage even
/// though it's actively working.
struct ProgressRanker<'a> {
    inner: &'a dyn reveal_cull::VisionRanker,
    app: &'a tauri::AppHandle,
    dir: &'a str,
    total: usize,
    done: std::sync::atomic::AtomicUsize,
}

impl reveal_cull::VisionRanker for ProgressRanker<'_> {
    fn rank_batch(
        &self,
        candidates: &[reveal_cull::RankCandidate],
    ) -> Result<Vec<reveal_cull::RankedResult>, reveal_cull::CullError> {
        let result = self.inner.rank_batch(candidates)?;
        let done = self
            .done
            .fetch_add(candidates.len(), std::sync::atomic::Ordering::Relaxed)
            + candidates.len();
        let _ = self.app.emit(
            "cull-progress",
            serde_json::json!({ "dir": self.dir, "phase": "cloud", "done": done, "total": self.total }),
        );
        Ok(result)
    }
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct CullResult {
    dir: String,
    considered: usize,
    survivors: usize,
    picked: usize,
    marked: usize,
    exported_to: Option<String>,
}

/// Auto-cull one just-imported day-folder down to the best `ai_cull_target`
/// frames and export them — the AI-picks step between `import_card`'s
/// preset application and whichever of the two independent outcomes are
/// enabled (mark into the story, export to the Desktop, or both — see
/// `read_ai_cull_prefs`). Structured like `import_card`/`export_photos`: an
/// in-flight guard, progress events, a final result event. Local prefilter
/// first (`reveal_cull::prefilter`), then a cloud vision ranking pass
/// (`reveal_cull::rank_all`), then the enabled outcome(s) — no round-trip
/// back to the frontend for either.
#[tauri::command]
pub(crate) async fn ai_cull(
    app: tauri::AppHandle,
    engine_state: tauri::State<'_, EngineState>,
    cull_state: tauri::State<'_, CullState>,
    cancel_state: tauri::State<'_, CullCancelState>,
    dir: String,
    paths: Vec<String>,
) -> Result<CullResult, String> {
    {
        let mut running = cull_state.0.lock().unwrap();
        if running.contains(&dir) {
            return Err("culling already in progress for this folder".into());
        }
        running.insert(dir.clone());
    }
    cancel_state.0.store(false, std::sync::atomic::Ordering::Relaxed);

    let (mark_story, export_desktop, target, api_key, model, provider) = read_ai_cull_prefs(&app);
    if !mark_story && !export_desktop {
        cull_state.0.lock().unwrap().remove(&dir);
        return Err("AI culling disabled".into());
    }
    let Some(api_key) = api_key else {
        cull_state.0.lock().unwrap().remove(&dir);
        return Err("no vision API key configured".into());
    };

    let _ = app.emit(
        "cull-started",
        serde_json::json!({ "dir": dir, "total": paths.len() }),
    );

    let engine = engine_state.0.clone();
    let cancel = cancel_state.0.clone();
    let app_for_worker = app.clone();
    let dir_key = dir.clone();

    let worker = tauri::async_runtime::spawn_blocking(move || -> Result<CullResult, String> {
        let (picked, considered, survivors) =
            score_paths(&app_for_worker, &dir_key, &paths, target, &api_key, model, provider, &cancel)?;

        // Mark the picks into the story (the same `q` quick-collection) —
        // only ADDS, never removes, same rule as the manual culling button:
        // a photo already there is left alone rather than toggled out.
        let mut marked = 0usize;
        if mark_story {
            let dir_path = std::path::Path::new(&dir_key);
            let existing: std::collections::HashSet<String> =
                story::stems(dir_path).into_iter().collect();
            for path in &picked {
                let p = std::path::Path::new(path);
                let stem = p
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if !existing.contains(&stem) && story::toggle(dir_path, p).is_ok() {
                    marked += 1;
                }
            }
        }

        let exported_to = if export_desktop {
            let home = std::env::var("HOME").map_err(|e| e.to_string())?;
            let folder_name = std::path::Path::new(&dir_key)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "culled".to_string());
            let dest_dir = format!("{home}/Desktop/{folder_name}");
            export_batch(&engine, &app_for_worker, &cancel, &picked, &dest_dir, 2048, 0.04, "cull-progress")?;
            Some(dest_dir)
        } else {
            None
        };

        Ok(CullResult {
            dir: dir_key,
            considered,
            survivors,
            picked: picked.len(),
            marked,
            exported_to,
        })
    })
    .await;

    cull_state.0.lock().unwrap().remove(&dir);

    let result: Result<CullResult, String> = match worker {
        Ok(inner) => inner,
        Err(e) => Err(e.to_string()),
    };

    match &result {
        Ok(stats) => {
            let folder_name = std::path::Path::new(&stats.dir)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let outcome = match (&stats.exported_to, stats.marked) {
                (Some(dest), m) if m > 0 => format!("added to story, exported to {dest}"),
                (Some(dest), _) => format!("exported to {dest}"),
                (None, m) if m > 0 => "added to story".to_string(),
                (None, _) => "kept".to_string(),
            };
            let _ = notify_user(
                "Culling IA — Reveal".to_string(),
                format!("{folder_name}: {} of {} kept, {outcome}", stats.picked, stats.considered),
            )
            .await;
            let _ = app.emit("cull-finished", stats);
        }
        Err(message) => {
            let _ = app.emit(
                "cull-failed",
                serde_json::json!({ "dir": dir, "message": message }),
            );
        }
    }

    result
}

#[tauri::command]
pub(crate) fn cancel_cull(cancel_state: tauri::State<'_, CullCancelState>) {
    cancel_state.0.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[derive(Clone, serde::Serialize)]
pub(crate) struct CullSelectionResult {
    dir: String,
    considered: usize,
    survivors: usize,
    picked: Vec<String>,
}

/// The rail's manual "Culling IA" button: score `paths` (the current grid's
/// view) and return the picks — no rating bump, no export. Unlike `ai_cull`,
/// a click here is the user's explicit ask, so it runs regardless of the
/// walk-away `ai_cull_enabled` toggle; it still needs a configured vision
/// API key to actually score anything. The caller decides what to do with
/// the picks — the frontend adds each to the folder's quick collection, the
/// same story-note mechanism the `q` shortcut toggles.
#[tauri::command]
pub(crate) async fn ai_cull_selection(
    app: tauri::AppHandle,
    cull_state: tauri::State<'_, CullState>,
    cancel_state: tauri::State<'_, CullCancelState>,
    dir: String,
    paths: Vec<String>,
) -> Result<CullSelectionResult, String> {
    {
        let mut running = cull_state.0.lock().unwrap();
        if running.contains(&dir) {
            return Err("culling already in progress for this folder".into());
        }
        running.insert(dir.clone());
    }
    cancel_state.0.store(false, std::sync::atomic::Ordering::Relaxed);

    let (_mark_story, _export_desktop, target, api_key, model, provider) = read_ai_cull_prefs(&app);
    let Some(api_key) = api_key else {
        cull_state.0.lock().unwrap().remove(&dir);
        return Err("no vision API key configured (Settings → AI & Automation)".into());
    };

    let cancel = cancel_state.0.clone();
    let app_for_worker = app.clone();
    let dir_key = dir.clone();

    let worker = tauri::async_runtime::spawn_blocking(move || {
        score_paths(&app_for_worker, &dir_key, &paths, target, &api_key, model, provider, &cancel)
    })
    .await;

    cull_state.0.lock().unwrap().remove(&dir);

    let (picked, considered, survivors) = match worker {
        Ok(inner) => inner?,
        Err(e) => return Err(e.to_string()),
    };

    Ok(CullSelectionResult { dir, considered, survivors, picked })
}
