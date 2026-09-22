//! Per-photo metadata written to the sidecar: captions, tags, and AI tag suggestions.
//!
//! Lifted out of `lib.rs` unchanged — see that file's header.

use crate::*;

/// Caption (dc:description) editing — sidecar field, everything else kept.
#[tauri::command]
pub(crate) async fn save_caption(path: String, description: String) -> Result<(), String> {
    apple_photos::update_metadata(&path, |sidecar| {
        sidecar.description = if description.trim().is_empty() { None } else { Some(description) };
        Ok(())
    })
}

/// Tags (dc:subject) editing — sidecar field, everything else kept.
#[tauri::command]
pub(crate) async fn save_tags(path: String, tags: Vec<String>) -> Result<(), String> {
    apple_photos::update_metadata(&path, |sidecar| {
        sidecar.tags = tags;
        Ok(())
    })
}

/// AI-suggested keyword tags for one photo — reuses the same `ai_api_key`/
/// `ai_model` prefs and embedded-preview-JPEG path as AI cull, since it's the
/// same "cheap vision call on the camera preview" shape, just a different
/// prompt/tool. Read-only: the caller (InfoBlock) merges the suggestions into
/// its editable tag list and still has to call `save_tags` to persist.
#[tauri::command]
pub(crate) async fn generate_tags(app: tauri::AppHandle, path: String) -> Result<Vec<String>, String> {
    let (_, _, _, api_key, model, provider) = cull::read_ai_cull_prefs(&app);
    let Some(api_key) = api_key else {
        return Err("no vision API key configured (Settings → AI & Automation)".into());
    };
    let model = model.unwrap_or_else(|| provider.default_model().to_string());
    tauri::async_runtime::spawn_blocking(move || {
        let jpeg = reveal_cull::embedded_preview_jpeg(&path, 768)
            .ok_or_else(|| "could not read a preview image for this photo".to_string())?;
        let suggester = reveal_cull::tag_suggester(provider, api_key, model);
        suggester.suggest_tags(&jpeg).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}
