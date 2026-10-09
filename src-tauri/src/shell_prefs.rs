//! Shell preferences persisted by the Rust app shell.
//!
//! Stores user preferences in `<app_data>/prefs.json`.

use tauri::{AppHandle, Emitter, Manager};
use crate::tray::TrayMenuState;

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
pub struct ShellPrefs {
    pub auto_import: bool,
    pub focus_mode: bool,
    /// The folder photos import into. When `None`, the import flow falls
    /// back to `roots[0]` (the primary catalogue root) — preserving the
    /// pre-choice behavior. Set via the sidebar's "Set as import folder" context-menu action.
    pub import_dir: Option<String>,
    /// A saved preset's name, applied automatically to every photo as it
    /// lands from a card import. `None` = imported photos keep no recipe
    /// (today's behavior) until developed by hand.
    pub default_import_preset: Option<String>,
    // Cached display fields only — the API key itself never rides on this
    // struct (it isn't broadcast via `shell-prefs-changed`); see
    // `read_garden_api_key`/`write_garden_prefs`.
    pub garden_username: Option<String>,
    pub garden_tier: Option<String>,
}

pub fn prefs_file(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("prefs.json"))
}

pub fn read_shell_prefs(app: &AppHandle) -> ShellPrefs {
    let Ok(path) = prefs_file(app) else {
        return ShellPrefs::default();
    };
    let Ok(raw) = std::fs::read_to_string(path) else {
        return ShellPrefs::default();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return ShellPrefs::default();
    };

    ShellPrefs {
        auto_import: value
            .get("auto_import")
            .or_else(|| value.get("autoImport"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        focus_mode: value
            .get("focus_mode")
            .or_else(|| value.get("focusMode"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        import_dir: value.get("import_dir").and_then(|v| v.as_str()).map(str::to_string),
        default_import_preset: value
            .get("default_import_preset")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        garden_username: value.get("garden_username").and_then(|v| v.as_str()).map(str::to_string),
        garden_tier: value.get("garden_tier").and_then(|v| v.as_str()).map(str::to_string),
    }
}

pub fn write_shell_prefs(app: &AppHandle, prefs: &ShellPrefs) -> Result<(), String> {
    let path = prefs_file(app)?;
    let mut value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    if !value.is_object() {
        value = serde_json::json!({});
    }
    value["auto_import"] = serde_json::json!(prefs.auto_import);
    value["focus_mode"] = serde_json::json!(prefs.focus_mode);
    set_or_remove(&mut value, "import_dir", prefs.import_dir.as_deref());
    set_or_remove(&mut value, "default_import_preset", prefs.default_import_preset.as_deref());
    set_or_remove(&mut value, "garden_username", prefs.garden_username.as_deref());
    set_or_remove(&mut value, "garden_tier", prefs.garden_tier.as_deref());

    let raw = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    // std::fs::write truncates its target then writes — a write that fails
    // partway (e.g. disk full) leaves prefs.json zero-byte or truncated,
    // losing every OTHER preference along with whatever field this call
    // meant to change. Write-to-temp-then-rename fixes it the same way:
    // rename() is atomic on the same filesystem.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

pub fn set_or_remove(value: &mut serde_json::Value, key: &str, v: Option<&str>) {
    match v {
        Some(v) => value[key] = serde_json::json!(v),
        None => {
            if let Some(obj) = value.as_object_mut() {
                obj.remove(key);
            }
        }
    }
}

/// Shell preferences persisted by the Rust app shell.
#[tauri::command]
pub fn load_shell_prefs(app: AppHandle) -> ShellPrefs {
    read_shell_prefs(&app)
}

/// Set automatic card import preference.
#[tauri::command]
pub fn set_auto_import(app: AppHandle, enabled: bool) -> Result<ShellPrefs, String> {
    let mut prefs = read_shell_prefs(&app);
    prefs.auto_import = enabled;
    write_shell_prefs(&app, &prefs)?;
    let _ = app.emit("shell-prefs-changed", &prefs);
    if let Some(state) = app.try_state::<TrayMenuState>() {
        let _ = state.auto_import_item.set_checked(enabled);
    }
    Ok(prefs)
}

/// Set (or clear, with `name: None`) the preset auto-applied to every photo
/// as it lands from a card import.
#[tauri::command]
pub fn set_default_import_preset(app: AppHandle, name: Option<String>) -> Result<ShellPrefs, String> {
    let mut prefs = read_shell_prefs(&app);
    prefs.default_import_preset = name.filter(|n| !n.trim().is_empty());
    write_shell_prefs(&app, &prefs)?;
    let _ = app.emit("shell-prefs-changed", &prefs);
    Ok(prefs)
}

/// Toggle automatic card import preference.
#[tauri::command]
pub fn toggle_auto_import(app: AppHandle) -> Result<ShellPrefs, String> {
    let mut prefs = read_shell_prefs(&app);
    prefs.auto_import = !prefs.auto_import;
    write_shell_prefs(&app, &prefs)?;
    let _ = app.emit("shell-prefs-changed", &prefs);
    if let Some(state) = app.try_state::<TrayMenuState>() {
        let _ = state.auto_import_item.set_checked(prefs.auto_import);
    }
    Ok(prefs)
}

/// Set the folder photos import into (the sidebar's "Set as import folder"
/// action). An empty/blank path clears the choice, falling back
/// to `roots[0]`. The sidebar's accent dot reacts via `shell-prefs-changed`.
#[tauri::command]
pub fn set_import_dir(app: AppHandle, path: Option<String>) -> Result<ShellPrefs, String> {
    let mut prefs = read_shell_prefs(&app);
    prefs.import_dir = path
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());
    write_shell_prefs(&app, &prefs)?;
    let _ = app.emit("shell-prefs-changed", &prefs);
    Ok(prefs)
}

#[tauri::command]
pub fn load_preferences(app: AppHandle) -> serde_json::Value {
    prefs_file(&app)
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

#[tauri::command]
pub fn save_preferences(
    app: AppHandle,
    preferences: serde_json::Value,
) -> Result<(), String> {
    let path = prefs_file(&app)?;
    let mut current = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    let current_object = current
        .as_object_mut()
        .ok_or_else(|| "preferences file is not an object".to_string())?;
    let incoming = preferences
        .as_object()
        .ok_or_else(|| "preferences payload is not an object".to_string())?;
    for (key, value) in incoming {
        current_object.insert(key.clone(), value.clone());
    }
    let cache_limit = crate::apple_photos::cache_limit(&current)?;
    let raw = serde_json::to_string_pretty(&current).map_err(|e| e.to_string())?;
    std::fs::write(path, raw).map_err(|e| e.to_string())?;
    crate::apple_photos::set_cache_limit(cache_limit)
}
