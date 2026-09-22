#![allow(unexpected_cfgs)]

use rayon::prelude::*;
use tauri::http::Response as HttpResponse;
use tauri::ipc::Response as IpcResponse;
use tauri::{Emitter, Manager};

mod apple_photos;
mod daily_note;
mod photo_cache;
mod preset;
mod story;
mod xmp_preset;

#[cfg(target_os = "macos")]
mod macos;

#[derive(Clone, Default)]
struct FocusState(std::sync::Arc<std::sync::Mutex<bool>>);

#[derive(Clone, Default)]
struct FocusPresenceState(std::sync::Arc<std::sync::Mutex<std::collections::BTreeSet<String>>>);

#[derive(Clone, Default)]
struct OpenFileState(std::sync::Arc<std::sync::Mutex<Option<String>>>);

struct TrayMenuState {
    auto_import_item: tauri::menu::CheckMenuItem<tauri::Wry>,
    focus_item: tauri::menu::CheckMenuItem<tauri::Wry>,
}

#[derive(Clone, Default)]
struct ImportState(std::sync::Arc<std::sync::Mutex<std::collections::BTreeSet<String>>>);

/// The import's stop flag — one import runs at a time in practice, so a
/// single shared flag (reset when an import starts) covers the HUD's stop.
#[derive(Clone, Default)]
struct ImportCancelState(std::sync::Arc<std::sync::atomic::AtomicBool>);

#[derive(Clone, Debug, Default, serde::Deserialize, serde::Serialize)]
struct ShellPrefs {
    auto_import: bool,
    focus_mode: bool,
    /// The folder photos import into. When `None`, the import flow falls
    /// back to `roots[0]` (the primary catalogue root) — preserving the
    /// pre-choice behavior. Set via the sidebar's "Définir comme dossier
    /// d'import" context-menu action.
    import_dir: Option<String>,
    /// A saved preset's name, applied automatically to every photo as it
    /// lands from a card import. `None` = imported photos keep no recipe
    /// (today's behavior) until developed by hand.
    default_import_preset: Option<String>,
    // Cached display fields only — the API key itself never rides on this
    // struct (it isn't broadcast via `shell-prefs-changed`); see
    // `read_garden_api_key`/`write_garden_prefs`.
    garden_username: Option<String>,
    garden_tier: Option<String>,
}

/// The sidebar account row's state — mirrors Swift's `GardenAccountModel`.
#[derive(Clone, Debug, serde::Serialize)]
struct GardenAccountInfo {
    signed_in: bool,
    username: Option<String>,
    tier: Option<String>,
    notes_count: i64,
    total_views: i64,
}

impl GardenAccountInfo {
    fn signed_out() -> Self {
        Self { signed_in: false, username: None, tier: None, notes_count: 0, total_views: 0 }
    }
}

fn prefs_file(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("prefs.json"))
}

fn read_shell_prefs(app: &tauri::AppHandle) -> ShellPrefs {
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

pub fn show_import_panel(app: &tauri::AppHandle) {
    // A walk-away HUD: only worth surfacing when you're not already looking
    // at the main window (Francis: "le hud devrait s'afficher seulement
    // quand l'app reveal n'est pas ouverte"). CloseRequested below never
    // really closes "main" — it just hides it — so `is_visible()` is exactly
    // the "would this float redundantly over the app you're already in" check.
    let already_in_app = app
        .get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if already_in_app {
        return;
    }
    // Real WKWebView transparency (the panel's CSS `rgba(30,30,30,.82)` +
    // `backdrop-filter` showing the desktop through it, not an opaque
    // backing store) requires macOS's private `drawsBackground` WKWebView
    // key, which wry only sets when the `macos-private-api` Cargo feature
    // and `macOSPrivateApi: true` (tauri.conf.json) are both on — applied
    // once, at webview creation, before this window's content ever loads.
    // No per-show native call needed here; NSWindow-level opacity is
    // likewise handled unconditionally by tao from this window's own
    // `"transparent": true` config.
    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(panel) = app_handle.get_webview_window("import-panel") {
            if let Ok(Some(monitor)) = app_handle.primary_monitor() {
                let scale_factor = monitor.scale_factor();
                // Tall enough for the thumbnail row (64px thumb + its own
                // padding) plus the progress/footer rows beneath it — the
                // old 108px was sized before those rows existed and was
                // clipping them (Francis: "sa hauteur devrait être celle
                // pour fitter avec l'aperçu").
                let logical_size = tauri::LogicalSize::new(396.0, 148.0);
                let physical_size = logical_size.to_physical::<u32>(scale_factor);
                let screen_size = monitor.size();

                let padding = (24.0 * scale_factor) as u32;
                let x = screen_size.width.saturating_sub(physical_size.width).saturating_sub(padding);
                let y = screen_size.height.saturating_sub(physical_size.height).saturating_sub(padding);

                let _ = panel.set_position(tauri::PhysicalPosition::new(x, y));
                let _ = panel.set_size(physical_size);
            }
            let _ = panel.show();
        }
    });
}

fn write_shell_prefs(app: &tauri::AppHandle, prefs: &ShellPrefs) -> Result<(), String> {
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
    // meant to change (unlike story.rs's note saves, this file lives in the
    // local app-data dir, not the NAS, so the silent-NFS-writeback failure
    // that first surfaced this class of bug doesn't apply here — but a full
    // disk still does, same as everywhere else). Write-to-temp-then-rename
    // fixes it the same way: rename() is atomic on the same filesystem, so a
    // failed write leaves the existing prefs file completely untouched.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

fn set_or_remove(value: &mut serde_json::Value, key: &str, v: Option<&str>) {
    match v {
        Some(v) => value[key] = serde_json::json!(v),
        None => {
            if let Some(obj) = value.as_object_mut() {
                obj.remove(key);
            }
        }
    }
}

/// The signed-in key alone — kept out of `ShellPrefs` so it never rides on
/// the `shell-prefs-changed` broadcast the frontend listens to.
fn read_garden_api_key(app: &tauri::AppHandle) -> Option<String> {
    if let Ok(path) = prefs_file(app) {
        if let Ok(raw) = std::fs::read_to_string(path) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(key) = value.get("garden_api_key").and_then(|v| v.as_str()) {
                    if !key.is_empty() {
                        return Some(key.to_string());
                    }
                }
            }
        }
    }
    // Fallback: check vault's obsidian garden plugin config
    if let Ok(client) = reveal_publish::GardenClient::from_vault(&vault_path(app)) {
        return Some(client.api_key().to_string());
    }
    None
}

fn write_garden_prefs(
    app: &tauri::AppHandle,
    api_key: Option<&str>,
    username: Option<&str>,
    tier: Option<&str>,
) -> Result<(), String> {
    let path = prefs_file(app)?;
    let mut value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !value.is_object() {
        value = serde_json::json!({});
    }
    set_or_remove(&mut value, "garden_api_key", api_key);
    set_or_remove(&mut value, "garden_username", username);
    set_or_remove(&mut value, "garden_tier", tier);
    let raw = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
    std::fs::write(path, raw).map_err(|e| e.to_string())
}

/// Own signed-in key first (parity with Swift's `loadConfig()`), falling
/// back to the vault's Obsidian `garden` plugin config so publishing still
/// works before anyone signs in via the sidebar.
fn garden_client(app: &tauri::AppHandle) -> Result<reveal_publish::GardenClient, String> {
    if let Some(key) = read_garden_api_key(app) {
        let username = read_shell_prefs(app).garden_username.unwrap_or_else(|| "francis".into());
        return Ok(reveal_publish::GardenClient::from_key(
            reveal_publish::DEFAULT_API_URL,
            &key,
            &username,
        ));
    }
    reveal_publish::GardenClient::from_vault(&vault_path(app)).map_err(|e| e.to_string())
}

/// Verify a key and store it as Reveal's own signed-in credential, emitting
/// `garden-account-changed` for the sidebar. Shared by the paste-key command
/// and the browser deep-link callback — same outcome, two ways in.
async fn sign_in_with_key(app: &tauri::AppHandle, key: &str) -> Result<GardenAccountInfo, String> {
    let key = key.trim().to_string();
    if key.is_empty() {
        return Err("empty key".to_string());
    }
    let info = {
        let key = key.clone();
        tauri::async_runtime::spawn_blocking(move || {
            reveal_publish::GardenClient::verify_key(reveal_publish::DEFAULT_API_URL, &key)
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?
    };
    write_garden_prefs(app, Some(&key), Some(&info.username), info.tier.as_deref())?;
    let out = GardenAccountInfo {
        signed_in: true,
        username: Some(info.username),
        tier: info.tier,
        notes_count: info.notes_count,
        total_views: info.total_views,
    };
    let _ = app.emit("garden-account-changed", &out);
    Ok(out)
}

/// Verify a pasted API key and store it as Reveal's own signed-in
/// credential — the sidebar's sign-in popover (Swift `GardenAccountModel.signIn`).
#[tauri::command]
async fn garden_sign_in(app: tauri::AppHandle, api_key: String) -> Result<GardenAccountInfo, String> {
    sign_in_with_key(&app, &api_key).await
}

/// Sidebar "Sign out" — clears the stored key entirely (falls back to any
/// vault config, same as before anyone signed in).
#[tauri::command]
fn garden_sign_out(app: tauri::AppHandle) -> Result<GardenAccountInfo, String> {
    write_garden_prefs(&app, None, None, None)?;
    let out = GardenAccountInfo::signed_out();
    let _ = app.emit("garden-account-changed", &out);
    Ok(out)
}

/// Silent re-verify of a stored key on launch (Swift `refreshIfNeeded`) — a
/// network hiccup keeps the cached username/tier showing rather than
/// surfacing an error the instant the app opens.
#[tauri::command]
async fn garden_refresh(app: tauri::AppHandle) -> GardenAccountInfo {
    let Some(key) = read_garden_api_key(&app) else {
        return GardenAccountInfo::signed_out();
    };
    let cached = read_shell_prefs(&app);
    let result = {
        let key = key.clone();
        tauri::async_runtime::spawn_blocking(move || {
            reveal_publish::GardenClient::verify_key(reveal_publish::DEFAULT_API_URL, &key)
        })
        .await
    };
    match result {
        Ok(Ok(info)) => {
            let _ = write_garden_prefs(&app, Some(&key), Some(&info.username), info.tier.as_deref());
            GardenAccountInfo {
                signed_in: true,
                username: Some(info.username),
                tier: info.tier,
                notes_count: info.notes_count,
                total_views: info.total_views,
            }
        }
        _ => {
            let fallback_user = cached.garden_username.or_else(|| {
                reveal_publish::GardenClient::from_vault(&vault_path(&app)).ok().map(|c| c.username.clone())
            });
            GardenAccountInfo {
                signed_in: fallback_user.is_some(),
                username: fallback_user,
                tier: cached.garden_tier,
                notes_count: 0,
                total_views: 0,
            }
        },
    }
}

#[tauri::command]
fn load_preferences(app: tauri::AppHandle) -> serde_json::Value {
    prefs_file(&app)
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

#[tauri::command]
fn save_preferences(
    app: tauri::AppHandle,
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
    let cache_limit = apple_photos::cache_limit(&current)?;
    let raw = serde_json::to_string_pretty(&current).map_err(|e| e.to_string())?;
    std::fs::write(path, raw).map_err(|e| e.to_string())?;
    apple_photos::set_cache_limit(cache_limit)
}

fn setup_main_menu(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};

    let about = PredefinedMenuItem::about(app, Some("About Reveal"), None)?;
    let app_separator = PredefinedMenuItem::separator(app)?;
    let settings =
        MenuItem::with_id(app, "settings", "Settings…", true, Some("CmdOrCtrl+,"))?;
    let app_separator_2 = PredefinedMenuItem::separator(app)?;
    let hide = PredefinedMenuItem::hide(app, Some("Hide Reveal"))?;
    let quit = PredefinedMenuItem::quit(app, Some("Quit Reveal"))?;
    let app_menu = Submenu::with_items(
        app,
        "Reveal",
        true,
        &[&about, &app_separator, &settings, &app_separator_2, &hide, &quit],
    )?;

    let undo = PredefinedMenuItem::undo(app, None)?;
    let redo = PredefinedMenuItem::redo(app, None)?;
    let edit_separator = PredefinedMenuItem::separator(app)?;
    let cut = PredefinedMenuItem::cut(app, None)?;
    let copy = PredefinedMenuItem::copy(app, None)?;
    let paste = PredefinedMenuItem::paste(app, None)?;
    let select_all = PredefinedMenuItem::select_all(app, None)?;
    let edit_menu = Submenu::with_items(
        app,
        "Edit",
        true,
        &[&undo, &redo, &edit_separator, &cut, &copy, &paste, &select_all],
    )?;

    let import_card = MenuItem::with_id(app, "menu-import-card", "Import from Card", true, None::<&str>)?;
    let import_folder =
        MenuItem::with_id(app, "menu-import-folder", "Add Library Folder…", true, None::<&str>)?;
    let auto_import =
        MenuItem::with_id(app, "menu-auto-import", "Toggle Automatic Import", true, None::<&str>)?;
    let import_menu =
        Submenu::with_items(app, "Import", true, &[&import_card, &import_folder, &auto_import])?;

    let engine_none =
        MenuItem::with_id(app, "menu-engine-none", "None", true, None::<&str>)?;
    let engine_spektra =
        MenuItem::with_id(app, "menu-engine-spektra", "Spektra", true, None::<&str>)?;
    let engine_menu = Submenu::with_items(app, "Engine", true, &[&engine_none, &engine_spektra])?;
    let develop_separator = PredefinedMenuItem::separator(app)?;
    let reset =
        MenuItem::with_id(app, "menu-reset-develop", "Reset Development Settings", true, None::<&str>)?;
    let develop_menu =
        Submenu::with_items(app, "Develop", true, &[&engine_menu, &develop_separator, &reset])?;

    let export_selected =
        MenuItem::with_id(app, "menu-export", "Export Selected Photo", true, None::<&str>)?;
    let publish_selected =
        MenuItem::with_id(app, "menu-publish", "Send Selected Photo to Vault", true, None::<&str>)?;
    let export_menu =
        Submenu::with_items(app, "Export", true, &[&export_selected, &publish_selected])?;

    let minimize = PredefinedMenuItem::minimize(app, None)?;
    let close = PredefinedMenuItem::close_window(app, None)?;
    let window_separator = PredefinedMenuItem::separator(app)?;
    let contact =
        MenuItem::with_id(app, "menu-contact-sheet", "Contact Sheet", true, None::<&str>)?;
    let develop_panel =
        MenuItem::with_id(app, "menu-develop-panel", "Develop Panel", true, None::<&str>)?;
    let render_queue =
        MenuItem::with_id(app, "menu-render-queue", "Render Queue", true, None::<&str>)?;
    let shortcuts =
        MenuItem::with_id(app, "menu-shortcuts", "Keyboard Shortcuts", true, None::<&str>)?;
    let window_menu = Submenu::with_items(
        app,
        "Window",
        true,
        &[
            &minimize,
            &close,
            &window_separator,
            &contact,
            &develop_panel,
            &render_queue,
            &shortcuts,
        ],
    )?;

    let menu = Menu::with_items(
        app,
        &[&app_menu, &edit_menu, &import_menu, &develop_menu, &export_menu, &window_menu],
    )?;
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| match event.id().as_ref() {
        "settings" => {
            show_main_window(app);
            let _ = app.emit("open-settings-requested", ());
        }
        "menu-import-card" => {
            show_main_window(app);
            let _ = app.emit("import-first-card-requested", ());
        }
        "menu-auto-import" => {
            let _ = app.emit("toggle-auto-import-requested", ());
        }
        "menu-import-folder" => {
            show_main_window(app);
            let _ = app.emit("add-library-folder-requested", ());
        }
        "menu-engine-none" => {
            let _ = app.emit("menu-clear-develop-requested", ());
        }
        "menu-engine-spektra" => {
            let _ = app.emit("menu-enable-develop-requested", ());
        }
        "menu-reset-develop" => {
            let _ = app.emit("menu-reset-develop-requested", ());
        }
        "menu-export" => {
            let _ = app.emit("menu-export-requested", ());
        }
        "menu-publish" => {
            let _ = app.emit("menu-publish-requested", ());
        }
        "menu-contact-sheet" => show_main_window(app),
        "menu-develop-panel" => {
            let _ = app.emit("toggle-dev-panel-requested", ());
        }
        "menu-render-queue" => {
            let _ = app.emit("toggle-render-queue-requested", ());
        }
        "menu-shortcuts" => {
            let _ = app.emit("toggle-shortcuts-requested", ());
        }
        _ => {}
    });
    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        #[cfg(target_os = "macos")]
        {
            if let Ok(ns_window) = window.ns_window() {
                let _ = macos::traffic_lights::style(ns_window as *mut objc::runtime::Object);
            }
        }
    }

    let focus_enabled = app
        .try_state::<FocusState>()
        .map(|state| *state.0.lock().unwrap())
        .unwrap_or(false);
    if focus_enabled {
        let _ = apply_focus_mode(app, true);
    }
}

fn hide_main_window(app: &tauri::AppHandle) {
    #[cfg(target_os = "macos")]
    macos::focus_backdrop::hide();

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

fn toggle_window_visibility(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(false);
        if is_visible {
            hide_main_window(app);
        } else {
            show_main_window(app);
            let is_focus = app
                .try_state::<FocusState>()
                .map(|state| *state.0.lock().unwrap())
                .unwrap_or(false);
            if is_focus {
                let _ = apply_focus_mode(app, true);
            }
        }
    }
}

fn emit_focus_mode(app: &tauri::AppHandle, enabled: bool) {
    let _ = app.emit("focus-mode-changed", serde_json::json!({ "enabled": enabled }));
}

fn apply_focus_mode(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    apply_focus_backdrop(app, enabled)?;

    if let Some(state) = app.try_state::<TrayMenuState>() {
        let _ = state.focus_item.set_checked(enabled);
    }

    emit_focus_mode(app, enabled);
    Ok(())
}

fn apply_focus_backdrop(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let app_handle = app.clone();
        let run = move || {
            if enabled {
                let ns_win = app_handle
                    .get_webview_window("main")
                    .and_then(|w| w.ns_window().ok())
                    .map(|w| w as *mut objc::runtime::Object)
                    .unwrap_or(std::ptr::null_mut());
                let _ = macos::focus_backdrop::show_below(ns_win);
            } else {
                macos::focus_backdrop::hide();
            }
        };

        unsafe {
            use objc::{class, msg_send, sel, sel_impl};
            let is_main: bool = msg_send![class!(NSThread), isMainThread];
            if is_main {
                run();
            } else {
                let _ = app.run_on_main_thread(run);
            }
        }
    }
    Ok(())
}

fn toggle_focus_mode(app: &tauri::AppHandle) {
    let enabled = app
        .try_state::<FocusState>()
        .map(|state| {
            let mut value = state.0.lock().unwrap();
            *value = !*value;
            *value
        })
        .unwrap_or(false);
    if let Err(e) = apply_focus_mode(app, enabled) {
        let _ = app.emit("app-error", serde_json::json!({ "message": e }));
    }
    show_main_window(app);
}

fn toggle_macos_appearance() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("/usr/bin/osascript")
            .args([
                "-e",
                r#"tell application "System Events" to tell appearance preferences to set dark mode to not dark mode"#,
            ])
            .status()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Err("Cette commande est disponible seulement sur macOS".into())
    }
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::menu::{Menu, MenuItem, CheckMenuItemBuilder};
    use tauri::tray::TrayIconBuilder;

    let shell_prefs = read_shell_prefs(app.handle());

    let show = MenuItem::with_id(app, "show", "Contact Sheet", true, None::<&str>)?;
    let import = MenuItem::with_id(app, "import-card", "Importer depuis la carte", true, None::<&str>)?;
    
    let auto_import = CheckMenuItemBuilder::new("Basculer import auto")
        .id("auto-import")
        .checked(shell_prefs.auto_import)
        .build(app)?;

    let focus = CheckMenuItemBuilder::new("Mode Focus")
        .id("focus")
        .checked(shell_prefs.focus_mode)
        .build(app)?;

    let appearance = MenuItem::with_id(app, "appearance", "Basculer clair/sombre", true, None::<&str>)?;
    let hide = MenuItem::with_id(app, "hide", "Masquer Reveal", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &import, &auto_import, &focus, &appearance, &hide, &quit])?;

    app.manage(TrayMenuState {
        auto_import_item: auto_import.clone(),
        focus_item: focus.clone(),
    });

    let mut builder = TrayIconBuilder::new()
        .tooltip("Reveal")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "hide" => hide_main_window(app),
            "focus" => toggle_focus_mode(app),
            "appearance" => {
                if let Err(e) = toggle_macos_appearance() {
                    let _ = app.emit("app-error", serde_json::json!({ "message": e }));
                }
            }
            "import-card" => {
                show_main_window(app);
                let _ = app.emit("import-first-card-requested", serde_json::json!({}));
            }
            "auto-import" => {
                show_main_window(app);
                let _ = app.emit("toggle-auto-import-requested", serde_json::json!({}));
            }
            "quit" => app.exit(0),
            _ => {}
        });

    // The menu bar wants a monochrome template glyph, not the app's coloured
    // tile — macOS recolors a template to match the bar's light/dark appearance,
    // so it stays a quiet aperture instead of a heavy dark square (Reveal's
    // whole "don't demand attention" intent). Falls back to the window icon if
    // the template can't be decoded.
    match tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png")) {
        Ok(icon) => {
            builder = builder.icon(icon).icon_as_template(true);
        }
        Err(e) => {
            eprintln!("tray template icon: {e}");
            if let Some(icon) = app.default_window_icon() {
                builder = builder.icon(icon.clone());
            }
        }
    }

    builder.build(app)?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn start_card_watcher(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut previous: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        loop {
            let cards = reveal_import::find_cards();
            let current: std::collections::BTreeSet<String> =
                cards.iter().map(|c| c.dcim.clone()).collect();

            if current != previous {
                let _ = app.emit("cards-changed", &cards);
                for card in cards.iter().filter(|card| !previous.contains(&card.dcim)) {
                    let _ = app.emit("card-mounted", card);
                }
                previous = current;
            }

            std::thread::sleep(std::time::Duration::from_secs(3));
        }
    });
}

/// Shell preferences persisted by the Rust app shell.
#[tauri::command]
fn load_shell_prefs(app: tauri::AppHandle) -> ShellPrefs {
    read_shell_prefs(&app)
}

/// Set automatic card import preference.
#[tauri::command]
fn set_auto_import(app: tauri::AppHandle, enabled: bool) -> Result<ShellPrefs, String> {
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
fn set_default_import_preset(app: tauri::AppHandle, name: Option<String>) -> Result<ShellPrefs, String> {
    let mut prefs = read_shell_prefs(&app);
    prefs.default_import_preset = name.filter(|n| !n.trim().is_empty());
    write_shell_prefs(&app, &prefs)?;
    let _ = app.emit("shell-prefs-changed", &prefs);
    Ok(prefs)
}

/// Toggle automatic card import preference.
#[tauri::command]
fn toggle_auto_import(app: tauri::AppHandle) -> Result<ShellPrefs, String> {
    let mut prefs = read_shell_prefs(&app);
    prefs.auto_import = !prefs.auto_import;
    write_shell_prefs(&app, &prefs)?;
    let _ = app.emit("shell-prefs-changed", &prefs);
    if let Some(state) = app.try_state::<TrayMenuState>() {
        let _ = state.auto_import_item.set_checked(prefs.auto_import);
    }
    Ok(prefs)
}

/// Set the folder photos import into (the sidebar's "Définir comme dossier
/// d'import" action). An empty/blank path clears the choice, falling back
/// to `roots[0]`. The sidebar's accent dot reacts via `shell-prefs-changed`.
#[tauri::command]
fn set_import_dir(app: tauri::AppHandle, path: Option<String>) -> Result<ShellPrefs, String> {
    let mut prefs = read_shell_prefs(&app);
    prefs.import_dir = path
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());
    write_shell_prefs(&app, &prefs)?;
    let _ = app.emit("shell-prefs-changed", &prefs);
    Ok(prefs)
}

/// Show a native macOS notification. Fallback no-op on other platforms.
#[tauri::command]
async fn notify_user(title: String, body: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification {} with title {}",
            applescript_string(&body),
            applescript_string(&title)
        );
        std::process::Command::new("/usr/bin/osascript")
            .args(["-e", &script])
            .status()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Open a path in Finder.
#[tauri::command]
async fn open_path(path: String) -> Result<(), String> {
    apple_photos::require_file(&path)?;
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("/usr/bin/open")
            .arg(path)
            .status()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Reveal a file in Finder with the file selected.
#[tauri::command]
async fn reveal_in_finder(path: String) -> Result<(), String> {
    apple_photos::require_file(&path)?;
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("/usr/bin/open")
            .args(["-R", &path])
            .status()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn applescript_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Toggle the system-wide macOS light/dark appearance.
#[tauri::command]
fn toggle_system_appearance() -> Result<(), String> {
    toggle_macos_appearance()
}

/// Toggle Reveal's focus mode. The frontend currently draws the overlay;
/// the native NSPanel backdrop is the next macOS-only refinement.
#[tauri::command]
fn toggle_focus(app: tauri::AppHandle) -> bool {
    let enabled = app
        .try_state::<FocusState>()
        .map(|state| {
            let mut value = state.0.lock().unwrap();
            *value = !*value;
            *value
        })
        .unwrap_or(false);
    let mut prefs = read_shell_prefs(&app);
    prefs.focus_mode = enabled;
    let _ = write_shell_prefs(&app, &prefs);
    let _ = app.emit("shell-prefs-changed", &prefs);
    if let Err(e) = apply_focus_mode(&app, enabled) {
        let _ = app.emit("app-error", serde_json::json!({ "message": e }));
    }
    enabled
}

/// Set Reveal's focus mode explicitly.
#[tauri::command]
fn set_focus(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let state = app
        .try_state::<FocusState>()
        .ok_or_else(|| "focus state unavailable".to_string())?;
    *state.0.lock().unwrap() = enabled;
    let mut prefs = read_shell_prefs(&app);
    prefs.focus_mode = enabled;
    write_shell_prefs(&app, &prefs)?;
    let _ = app.emit("shell-prefs-changed", &prefs);
    apply_focus_mode(&app, enabled)
}

/// Keep the native focus backdrop visible only while at least one Reveal
/// window is under the pointer. The delayed leave avoids a flash between the
/// main and Develop windows.
#[tauri::command]
fn set_focus_window_presence(
    app: tauri::AppHandle,
    window_id: String,
    inside: bool,
) -> Result<(), String> {
    let presence = app
        .try_state::<FocusPresenceState>()
        .ok_or_else(|| "focus presence state unavailable".to_string())?;
    {
        let mut windows = presence.0.lock().unwrap();
        if inside {
            windows.insert(window_id);
        } else {
            windows.remove(&window_id);
        }
    }

    if inside {
        let enabled = app
            .try_state::<FocusState>()
            .map(|state| *state.0.lock().unwrap())
            .unwrap_or(false);
        if enabled {
            apply_focus_backdrop(&app, true)?;
        }
        return Ok(());
    }

    let delayed_app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(220));
        let still_outside = delayed_app
            .try_state::<FocusPresenceState>()
            .map(|state| state.0.lock().unwrap().is_empty())
            .unwrap_or(true);
        let enabled = delayed_app
            .try_state::<FocusState>()
            .map(|state| *state.0.lock().unwrap())
            .unwrap_or(false);
        if still_outside && enabled {
            // The backdrop hide touches AppKit (NSWindow orderOut:), which
            // must happen on the main thread — calling it from this background
            // thread crashed the app with EXC_BREAKPOINT inside WindowManagement.
            // Hop to the main queue for the AppKit call; everything above is
            // plain state reads through `try_state` and is thread-safe.
            #[cfg(target_os = "macos")]
            {
                let main_app = delayed_app.clone();
                let scheduler = delayed_app.clone();
                let _ = scheduler.run_on_main_thread(move || {
                    let _ = apply_focus_backdrop(&main_app, false);
                });
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = apply_focus_backdrop(&delayed_app, false);
            }
        }
    });
    Ok(())
}

/// Current focus state for initial frontend hydration.
#[tauri::command]
fn focus_state(state: tauri::State<'_, FocusState>) -> bool {
    *state.0.lock().unwrap()
}

/// Lightroom-style borderless fullscreen for the main window: cover the whole
/// display in place (menu bar + Dock hidden) rather than the macOS native
/// Spaces fullscreen. On exit, restore the pre-fullscreen frame.
#[tauri::command]
fn set_simple_fullscreen(window: tauri::WebviewWindow, enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let ns_window = window.ns_window().map_err(|e| e.to_string())? as *mut objc::runtime::Object;
        if enabled {
            macos::simple_fullscreen::enter(ns_window)?;
        } else {
            macos::simple_fullscreen::exit(ns_window)?;
        }
        let _ = window.set_focus();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (&window, enabled);
    }
    Ok(())
}

/// Show the main Reveal window from JS or tray.
#[tauri::command]
fn show_contact_sheet(app: tauri::AppHandle) {
    show_main_window(&app);
}

/// Hide the main Reveal window while keeping the app alive.
#[tauri::command]
fn hide_contact_sheet(app: tauri::AppHandle) {
    hide_main_window(&app);
}

/// IPC smoke test — proves invoke() reaches Rust.
#[tauri::command]
fn ping() -> String {
    format!(
        "pong — reveal_lib {} (decode {}, engine {})",
        env!("CARGO_PKG_VERSION"),
        reveal_decode::VERSION,
        reveal_engine::VERSION,
    )
}

/// Dev/test hook: `REVEAL_AUTODEV=<raw path>` makes the frontend develop
/// that file on mount — lets the whole webview→IPC→engine round trip be
/// verified headlessly from the app's log.
#[tauri::command]
fn autoload_path() -> Option<String> {
    if let Some(path) = std::env::var("REVEAL_AUTODEV").ok().filter(|s| !s.is_empty()) {
        return Some(path);
    }
    std::env::args()
        .skip(1)
        .find(|path| {
            matches!(
                std::path::Path::new(path).extension().and_then(|ext| ext.to_str()).map(str::to_ascii_lowercase).as_deref(),
                Some("jpg" | "jpeg")
            )
        })
}

#[tauri::command]
fn take_open_file(state: tauri::State<'_, OpenFileState>) -> Option<String> {
    state.0.lock().unwrap().take()
}

struct EditorInfo {
    name: &'static str,
    ids: &'static [&'static str],
}

const CANDIDATES: &[EditorInfo] = &[
    EditorInfo { name: "Photoshop", ids: &["com.adobe.Photoshop"] },
    EditorInfo { name: "Lightroom Classic", ids: &["com.adobe.LightroomClassicCC7", "com.adobe.LightroomClassic"] },
    EditorInfo { name: "Lightroom", ids: &["com.adobe.lightroomCC"] },
    EditorInfo { name: "Capture One", ids: &["com.captureone.captureone16", "com.captureone.captureone15", "com.phaseone.captureone", "com.captureone.captureone"] },
    EditorInfo { name: "Affinity Photo", ids: &["com.seriflabs.affinityphoto2", "com.seriflabs.affinityphoto", "com.canva.affinity"] },
    EditorInfo { name: "Pixelmator Pro", ids: &["com.pixelmatorteam.pixelmator.x"] },
    EditorInfo { name: "Luminar Neo", ids: &["com.skylum.luminarneo", "com.skylum.luminarAI", "com.skylum.luminar4"] },
    EditorInfo { name: "DxO PhotoLab", ids: &["com.dxo.PhotoLab8", "com.dxo.PhotoLab7", "com.dxo.PhotoLab6", "com.dxo.PhotoLab5", "com.dxo.PhotoLab4"] },
    EditorInfo { name: "ON1 Photo RAW", ids: &["com.ononesoftware.ON1PhotoRAW2025.premium", "com.ononesoftware.ON1PhotoRAW2024.premium", "com.ononesoftware.ON1PhotoRAW2023.premium"] },
    EditorInfo { name: "darktable", ids: &["org.darktable.darktable", "photos.darktable.darktable"] },
    EditorInfo { name: "RawTherapee", ids: &["com.rawtherapee.RawTherapee"] },
    EditorInfo { name: "GIMP", ids: &["org.gimp.gimp-2.10", "org.gimp.GIMP", "org.gimp.gimp"] },
];

#[tauri::command]
async fn list_external_editors() -> Vec<(String, String)> {
    let mut installed = Vec::new();
    for candidate in CANDIDATES {
        for id in candidate.ids {
            let output = std::process::Command::new("mdfind")
                .arg(format!("kMDItemCFBundleIdentifier == \"{}\"", id))
                .output();
            if let Ok(out) = output {
                if out.status.success() {
                    let stdout_str = String::from_utf8_lossy(&out.stdout);
                    if let Some(path) = stdout_str.lines().next() {
                        let path = path.trim().to_string();
                        if !path.is_empty() {
                            installed.push((candidate.name.to_string(), path));
                            break;
                        }
                    }
                }
            }
        }
    }
    installed
}

#[tauri::command]
async fn open_in_editor(file_path: String, app_path: String) -> Result<(), String> {
    apple_photos::require_file(&file_path)?;
    std::process::Command::new("open")
        .args(&["-a", &app_path, &file_path])
        .status()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn load_catalog_note(root: String) -> String {
    let path = std::path::Path::new(&root).join("reveal.md");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|_| "---\ntype: reveal-catalog\ncreated: 2026-07-16\n---\n\n".to_string())
}

#[tauri::command]
async fn save_catalog_note(root: String, content: String) -> Result<(), String> {
    let path = std::path::Path::new(&root).join("reveal.md");
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

/// Native RAW file picker (Rust-side dialog — no JS plugin surface needed).
#[tauri::command]
async fn pick_raw(app: tauri::AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    app.dialog()
        .file()
        .add_filter(
            "Photos RAW",
            &[
                "raf", "dng", "nef", "arw", "cr2", "cr3", "orf", "rw2", "pef", "srw",
            ],
        )
        .blocking_pick_file()
        .and_then(|f| f.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Develop a RAW through the film pipeline, return JPEG bytes (binary IPC —
/// zero JSON, zero base64). `max_px` bounds the preview's long edge.
fn developed_preview_source_key(path: &std::path::Path) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut hasher);
    hasher.finish()
}

fn developed_preview_cache_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
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
fn preview_sidecar_path(source: &std::path::Path) -> Option<std::path::PathBuf> {
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
fn companion_jpeg_path(source: &std::path::Path) -> Option<std::path::PathBuf> {
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
fn downscale_grid_thumb(bytes: Vec<u8>, max_edge: u32) -> Vec<u8> {
    let orientation = match exif::Reader::new().read_from_container(&mut std::io::Cursor::new(&bytes)) {
        Ok(exif_data) => match exif_data.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
            Some(field) => match field.value.get_uint(0) {
                Some(v @ 1..=8) => v,
                _ => 1,
            },
            None => 1,
        },
        Err(_) => 1,
    };

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
fn served_preview_mtime(source: &std::path::Path) -> u64 {
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
fn write_sidecar_if_changed(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
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
const DURABLE_PREVIEW_EDGE: u32 = 2048;

/// Persist a served thumbnail as the durable `.preview.jpg`, but ONLY when it
/// was rendered at the size that file is defined to hold.
///
/// The protocol serves the size its caller asked for — 640 for contact-sheet
/// cells, 2048 for a single-photo view — and both used to land here. That
/// made the grid and the viewer overwrite each other's sidecar on every
/// visit, rewriting the file back and forth across the network mount, and
/// left whichever came last as the "durable" preview that publish would
/// upload. Serving small and persisting small are different decisions.
fn persist_thumb_cache(source: &std::path::Path, bytes: &[u8], size: u32) {
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
struct ThumbSemaphore {
    count: std::sync::Mutex<usize>,
    cv: std::sync::Condvar,
    max: usize,
}

impl ThumbSemaphore {
    fn new(max: usize) -> Self {
        Self { count: std::sync::Mutex::new(0), cv: std::sync::Condvar::new(), max }
    }

    fn acquire(&self) {
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

struct ThumbConcurrencyState(std::sync::Arc<ThumbSemaphore>);

/// RAII guard: acquired before a thumb decode, released (even on early
/// return/panic-unwind) when the request finishes.
struct ThumbPermit<'a>(&'a ThumbSemaphore);

impl<'a> ThumbPermit<'a> {
    fn acquire(sem: &'a ThumbSemaphore) -> Self {
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
const PREVIEW_CACHE_BUDGET_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// Coalesces concurrent prune requests: a render storm schedules at most one
/// running prune at a time instead of one per frame.
static CACHE_PRUNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Fire-and-forget the LRU prune off the render path, guarded so overlapping
/// renders never stack prunes.
fn schedule_cache_prune(app: &tauri::AppHandle) {
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
fn prune_preview_cache(dir: &std::path::Path, budget_bytes: u64) {
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
struct PreviewCacheStatus {
    size_bytes: u64,
    photo_count: usize,
    /// The disk allowance. The photo count is still reported because it is
    /// what a photographer thinks in, but it is an OUTCOME now, not a limit:
    /// how many photos fit depends on their size.
    limit_bytes: u64,
}

#[derive(serde::Serialize)]
struct PreviewCacheCleanup {
    removed_bytes: u64,
}

#[tauri::command]
async fn developed_preview_cache_status(app: tauri::AppHandle) -> Result<PreviewCacheStatus, String> {
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
async fn developed_preview_cache_clear(app: tauri::AppHandle) -> Result<PreviewCacheCleanup, String> {
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
fn cache_developed_preview_locally(
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
const GRID_PREVIEW_EDGE: u32 = 768;

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
fn developed_preview_cache_path(
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
fn newest_cached_render(
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
fn newest_matching(dir: &std::path::Path, prefix: &str) -> Option<std::path::PathBuf> {
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
fn developed_preview_cache_name(path: &std::path::Path, max_px: u32, version: u64) -> String {
    format!(
        "{:016x}-{max_px}-{version:x}.jpg",
        developed_preview_source_key(path)
    )
}

/// Shared by `develop_preview` (returns bytes to the frontend) and
/// `copy_developed_preview_to_clipboard` (writes bytes straight to
/// NSPasteboard) — same cache-or-develop logic either way.
async fn developed_preview_jpeg(
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
async fn develop_preview(
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
async fn copy_developed_preview_to_clipboard(
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
async fn copy_photo_preview_to_clipboard(path: String) -> Result<(), String> {
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
static PUBLISH_GENERATION: std::sync::OnceLock<
    std::sync::Mutex<std::collections::HashMap<String, u64>>,
> = std::sync::OnceLock::new();

/// Claim the right to publish this photo, invalidating any in-flight publish.
fn next_publish_generation(path: &str) -> u64 {
    let map = PUBLISH_GENERATION.get_or_init(Default::default);
    let mut guard = map.lock().unwrap_or_else(|e| e.into_inner());
    let slot = guard.entry(path.to_string()).or_insert(0);
    *slot += 1;
    *slot
}

/// Is this publish still the newest one claimed for the photo?
fn publish_generation_is_current(path: &str, generation: u64) -> bool {
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
fn write_preview_sidecar_bytes(
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
    let grid = (max_px > GRID_PREVIEW_EDGE).then(|| downscale_grid_thumb(jpeg.to_vec(), GRID_PREVIEW_EDGE));
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
fn write_cache_entry(dest: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = dest.with_extension("part");
    match std::fs::write(&tmp, bytes).and_then(|()| std::fs::rename(&tmp, dest)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

#[derive(serde::Serialize)]
struct DevelopRgbaResult {
    width: u32,
    height: u32,
    render_ms: u128,
    decode_ms: u128,
    rgba: Vec<u8>,
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
async fn develop_preview_rgba(
    app: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
    path: String,
    recipe: reveal_engine::Recipe,
    max_px: u32,
    live: Option<bool>,
) -> Result<DevelopRgbaResult, String> {
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

    Ok(DevelopRgbaResult {
        width: out.width,
        height: out.height,
        render_ms: out.render_ms,
        decode_ms: out.decode_ms,
        rgba: out.rgba,
    })
}

/// For engines whose interactive path is NOT JPEG (Rapid's canvas/RGBA proxy):
/// render one via the engine-agnostic `develop_jpeg` (dispatches through the
/// registry by `recipe.engine`, so this is correct for any engine) and pass it
/// through the one true funnel, `write_preview_sidecar_bytes`. This is the
/// pattern any FUTURE non-JPEG interactive engine must follow too.
async fn write_preview_sidecar(
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

/// Native folder picker for the cull grid.
#[tauri::command]
async fn pick_folder(app: tauri::AppHandle) -> Option<String> {
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
fn is_writable_dir(path: &std::path::Path) -> bool {
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
struct FrameInfo {
    path: String,
    name: String,
    rating: u8,
}

/// The RAW frames of one folder (non-recursive), with sidecar ratings.
#[tauri::command]
async fn list_dir(path: String) -> Result<Vec<FrameInfo>, String> {
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
async fn set_rating(
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
async fn scan_root(
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
async fn scan_folder(
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
async fn move_photo(path: String, dest_dir: String) -> Result<String, String> {
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
async fn rename_dir(path: String, new_name: String) -> Result<String, String> {
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
async fn create_dir(parent_dir: String, name: String) -> Result<String, String> {
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
async fn move_dir(path: String, dest_parent_dir: String) -> Result<String, String> {
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
fn index_dirs(
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
async fn add_catalog_root(
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
async fn park_working_frame(
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
async fn release_working_frame(state: tauri::State<'_, EngineState>) -> Result<(), String> {
    let engine = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || engine.clear_working());
    Ok(())
}

/// The file's bytes, if it is an ordinary image rather than a RAW.
///
/// A Google Takeout export hands back JPEGs still carrying a `.DNG`
/// extension — 113 in one folder here, 4032x3024, which libraw cannot touch,
/// so every RAW branch refuses them and the photo reads as "Preview
/// unavailable". Decoding is attempted, not assumed: a truly broken file
/// must still be reported as broken rather than served as bytes.
fn plain_image_bytes(path: &std::path::Path) -> Option<Vec<u8>> {
    let bytes = std::fs::read(path).ok()?;
    image::load_from_memory(&bytes).ok()?;
    Some(bytes)
}

/// Can we reach the folder this photo lives in right now?
///
/// The UI marks a photo as coming from cache while its source is unreachable;
/// this is how it learns the archive came back, without waiting for the next
/// failure to tell it.
#[tauri::command]
fn source_reachable(path: String) -> bool {
    let p = std::path::Path::new(&path);
    apple_photos::is_asset(&path) || is_volume_mounted(p)
}

/// Every registered library, with its frame count and whether its folder is
/// reachable right now. Drives the Libraries tab in Settings.
#[tauri::command]
async fn catalog_roots(
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
fn remove_catalog_root(
    index: tauri::State<'_, IndexState>,
    path: String,
) -> Result<usize, String> {
    index.0.remove_root(&path).map_err(|e| e.to_string())
}

/// Frames of one indexed folder, filtered by minimum rating.
#[tauri::command]
async fn index_frames(
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

struct IndexState(std::sync::Arc<reveal_index::Index>);

/// Lets the importer remember, in the catalogue, what it has already hashed.
///
/// Without it, confirming that a re-imported frame is a duplicate means
/// reading the archived copy back off the NAS in full — 2.11s for a 43 MB
/// frame, measured, and by far the largest cost of re-importing a card that
/// is mostly already archived. With it, that read happens once in a file's
/// life and every later import gets a cryptographic answer for free.
struct CatalogueHashes(std::sync::Arc<reveal_index::Index>);

impl reveal_import::HashCache for CatalogueHashes {
    fn get(&self, path: &std::path::Path) -> Option<String> {
        self.0.content_hash(&path.to_string_lossy())
    }
    fn put(&self, path: &std::path::Path, hash: &str) {
        // Bookkeeping: losing it costs the next import one re-read, nothing
        // more, so it must never interrupt an import that is otherwise fine.
        if let Err(e) = self.0.set_content_hash(&path.to_string_lossy(), hash) {
            eprintln!("import: could not cache hash for {}: {e}", path.display());
        }
    }
}
struct ExportState(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Default for ExportState {
    fn default() -> Self {
        Self(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)))
    }
}

/// One AI cull runs at a time per day-folder — mirrors `ImportState`'s
/// in-flight guard so triggering it twice for the same folder (e.g. a
/// re-emitted `import-finished`) is a clear error instead of a data race.
#[derive(Clone, Default)]
struct CullState(std::sync::Arc<std::sync::Mutex<std::collections::BTreeSet<String>>>);

struct CullCancelState(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Default for CullCancelState {
    fn default() -> Self {
        Self(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)))
    }
}

/// Where a developed JPEG should land inside the Obsidian vault so notes can
/// reference it — the vault's own `attachmentFolderPath` (read from
/// `.obsidian/app.json`), resolved against the vault root. Falls back to the
/// vault root when unset or set to note-relative (`./`), and creates it. This
/// is a filesystem export INTO the vault, distinct from Garden publishing.
#[tauri::command]
fn vault_attachment_dir(app: tauri::AppHandle) -> Result<String, String> {
    let vault = vault_path(&app);
    if !vault.exists() {
        return Err(format!(
            "Obsidian vault path does not exist: '{}'. Please configure a valid vault in Settings.",
            vault.display()
        ));
    }
    let rel = std::fs::read_to_string(vault.join(".obsidian/app.json"))
        .ok()
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
        .and_then(|v| {
            v.get("attachmentFolderPath")
                .and_then(|x| x.as_str())
                .map(str::to_string)
        })
        .unwrap_or_default();
    // "" or "/" → vault root; "./" or "./x" is note-relative and has no single
    // path, so we also fall back to the root there.
    let dir = if rel.is_empty() || rel == "/" || rel.starts_with("./") {
        vault
    } else {
        vault.join(rel.trim_start_matches('/'))
    };
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Could not create attachment directory '{}': {e}", dir.display()))?;
    Ok(dir.to_string_lossy().into_owned())
}

/// The vault path (for Garden credentials) — pref with a sane default.
fn vault_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    let prefs = app
        .path()
        .app_data_dir()
        .map(|d| d.join("prefs.json"))
        .ok();
    if let Some(p) = prefs {
        if let Ok(raw) = std::fs::read_to_string(&p) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(vault) = v
                    .get("vault")
                    .and_then(|x| x.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    return std::path::PathBuf::from(vault);
                }
            }
        }
    }
    dirs_home(Some(app)).join("Documents/Atelier")
}

fn dirs_home(app: Option<&tauri::AppHandle>) -> std::path::PathBuf {
    if let Some(app) = app {
        if let Ok(home) = app.path().home_dir() {
            return home;
        }
    }
    std::env::var("HOME").map(std::path::PathBuf::from).unwrap_or_default()
}

/// Photo stems currently marked into the folder's story note.
#[tauri::command]
async fn story_stems(dir: String) -> Vec<String> {
    story::stems(std::path::Path::new(&dir))
}

/// Which of these folders carry a story note with photos — the sidebar's
/// red "cette journée a une histoire" dots (Swift: `folderHasStory`).
#[tauri::command]
async fn story_dirs(dirs: Vec<String>) -> Vec<String> {
    dirs.into_iter()
        .filter(|d| !story::stems(std::path::Path::new(d)).is_empty())
        .collect()
}

/// Toggle a photo in the folder's story. Returns the new stems.
#[tauri::command]
async fn story_toggle(dir: String, path: String) -> Result<Vec<String>, String> {
    story::toggle(std::path::Path::new(&dir), std::path::Path::new(&path))
        .map_err(|e| e.to_string())
}

/// Load the raw markdown story note.
#[tauri::command]
async fn load_story_note(dir: String) -> String {
    let dirp = std::path::Path::new(&dir);
    let path = story::note_path(dirp);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|_| "---\npublish: true\n---\n\n".to_string())
}

/// Save the story note's body, preserving the on-disk frontmatter verbatim.
/// Read-modify-write from disk (invariant 3): the composer sends serialized
/// frontmatter + body, but only its BODY is trusted — the disk note's
/// frontmatter is kept as-is so a body save can never clobber a concurrent
/// theme/pin write that landed between the composer's load and its save.
#[tauri::command]
async fn save_story_note(dir: String, content: String) -> Result<(), String> {
    let dirp = std::path::Path::new(&dir);
    // Parse the incoming content to extract just its body (the composer's blocks).
    let incoming = story::StoryNote::parse(&content, &story::note_path(dirp));
    // Load the current on-disk note and replace only its body.
    let mut note = story::StoryNote::load(dirp);
    note.body = incoming.body;
    note.save().map_err(|e| e.to_string())
}

/// Read the story note's theme tokens (None = unspecified, inherits default).
#[tauri::command]
async fn story_load_theme(dir: String) -> story::ThemeTokens {
    story::StoryNote::load(std::path::Path::new(&dir)).theme_tokens()
}

/// Write the theme tokens to the note's frontmatter, deriving light + fg.
/// Read-modify-write from disk — never from an in-memory copy (invariant 3).
#[tauri::command]
async fn story_set_theme(dir: String, tokens: story::ThemeTokens) -> Result<(), String> {
    let mut note = story::StoryNote::load(std::path::Path::new(&dir));
    note.set_theme(&tokens);
    note.save().map_err(|e| e.to_string())
}

/// Set or clear pin state on the story note. Read-modify-write from disk.
#[tauri::command]
async fn story_set_pinned(dir: String, pinned: bool, pinned_at: Option<String>) -> Result<(), String> {
    let mut note = story::StoryNote::load(std::path::Path::new(&dir));
    note.set_pinned(pinned, pinned_at.as_deref());
    note.save().map_err(|e| e.to_string())
}

/// Scan folders for story notes — feeds the ÉPINGLÉES + RÉCENTES lists.
#[tauri::command]
async fn list_story_notes(dirs: Vec<String>) -> Vec<story::StoryNoteInfo> {
    story::list_story_notes(&dirs)
}

fn parse_markdown_story(raw: &str, dirp: &std::path::Path) -> (String, String) {
    let mut title = dirp.file_name().unwrap_or_default().to_string_lossy().to_string();
    let body;
    let text = raw.replace("\r\n", "\n");
    if text.starts_with("---\n") {
        if let Some(end) = text[4..].find("\n---") {
            let fm_section = &text[4..4 + end];
            for line in fm_section.lines() {
                if let Some(rest) = line.strip_prefix("title:") {
                    title = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                }
            }
            body = text[4 + end + 4..].to_string();
        } else {
            body = text;
        }
    } else {
        body = text;
    }
    (title, body)
}

fn markdown_to_html(body: &str, urls: &[(String, String)]) -> String {
    let mut html = String::new();
    let paragraphs = body.split("\n\n");
    for p in paragraphs {
        let p_trimmed = p.trim();
        if p_trimmed.is_empty() {
            continue;
        }
        if p_trimmed.starts_with("# ") {
            html.push_str(&format!("<h1>{}</h1>\n", &p_trimmed[2..]));
        } else if p_trimmed.starts_with("## ") {
            html.push_str(&format!("<h2>{}</h2>\n", &p_trimmed[3..]));
        } else if p_trimmed.starts_with("### ") {
            html.push_str(&format!("<h3>{}</h3>\n", &p_trimmed[4..]));
        } else {
            let mut has_embeds = false;
            let mut embeds_html = String::new();
            let mut rest = p_trimmed;
            while let Some(start) = rest.find("![[") {
                has_embeds = true;
                rest = &rest[start + 3..];
                if let Some(end) = rest.find("]]") {
                    let inner = &rest[..end];
                    let link = inner.split('|').next().unwrap_or(inner).trim();
                    let stem = link.strip_suffix(".jpg").or_else(|| link.strip_suffix(".jpeg")).unwrap_or(link);
                    if let Some((_, url)) = urls.iter().find(|(s, _)| s == stem) {
                        embeds_html.push_str(&format!("<img src=\"{}\" alt=\"{}\" />\n", url, stem));
                    }
                    rest = &rest[end + 2..];
                } else {
                    break;
                }
            }
            if has_embeds {
                html.push_str(&format!("<div class=\"photo-row\">\n{}</div>\n", embeds_html));
            } else {
                html.push_str(&format!("<p>{}</p>\n", p_trimmed.replace("\n", "<br>")));
            }
        }
    }
    html
}

/// Export the story folder to a local directory with index.html and developed JPEGs.
#[tauri::command]
async fn export_local_story(
    app: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
    dir: String,
    dest: String,
    long_edge: u32,
    border_frac: f32,
) -> Result<(), String> {
    let engine = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let dirp = std::path::Path::new(&dir);
        let destp = std::path::Path::new(&dest);
        let stems = story::stems(dirp);
        if stems.is_empty() {
            return Err("no photos in the story".to_string());
        }
        let images_dir = destp.join("images");
        std::fs::create_dir_all(&images_dir).map_err(|e| e.to_string())?;
        let total = stems.len();
        let emit = |done: usize, current: &str, phase: &str| {
            let _ = app.emit(
                "export-progress",
                serde_json::json!({ "done": done, "total": total, "current": current, "phase": phase }),
            );
        };
        let mut urls: Vec<(String, String)> = Vec::new();
        for (i, stem) in stems.iter().enumerate() {
            emit(i, stem, "developing");
            let raw = reveal_decode::RAW_EXTENSIONS
                .iter()
                .map(|e| dirp.join(format!("{stem}.{e}")))
                .chain(
                    reveal_decode::RAW_EXTENSIONS
                        .iter()
                        .map(|e| dirp.join(format!("{stem}.{}", e.to_uppercase()))),
                )
                .find(|p| p.exists())
                .ok_or_else(|| format!("RAW not found for {stem}"))?;
            let recipe = reveal_meta::read(&raw)
                .ok()
                .flatten()
                .and_then(|s| s.engine_settings)
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();
            let (jpeg, _, _) = engine
                .export_jpeg(&raw, &recipe, long_edge, border_frac)
                .map_err(|e| format!("develop {stem}: {e:#}"))?;
            let jpg_path = images_dir.join(format!("{stem}.jpg"));
            std::fs::write(&jpg_path, &jpeg).map_err(|e| e.to_string())?;
            urls.push((stem.clone(), format!("images/{stem}.jpg")));
        }
        let raw_md = std::fs::read_to_string(story::note_path(dirp)).map_err(|e| e.to_string())?;
        let (title, body) = parse_markdown_story(&raw_md, dirp);
        let body_html = markdown_to_html(&body, &urls);
        let html_content = format!(r#"<!DOCTYPE html>
<html lang="fr">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>{title}</title>
  <style>
    :root {{
      --bg: #15110d;
      --fg: #eae5de;
      --accent: #d6202c;
      --line: rgba(234, 229, 222, 0.1);
      --font-serif: "Newsreader", "Georgia", serif;
      --font-sans: "Inter", "Helvetica Neue", sans-serif;
    }}
    @media (prefers-color-scheme: light) {{
      :root {{
        --bg: #faf8f5;
        --fg: #1a1a1a;
        --line: rgba(0, 0, 0, 0.08);
      }}
    }}
    body {{
      background: var(--bg);
      color: var(--fg);
      font-family: var(--font-serif);
      margin: 0;
      padding: 3rem 1.5rem;
      line-height: 1.6;
      display: flex;
      flex-direction: column;
      align-items: center;
    }}
    main {{
      max-width: 50rem;
      width: 100%;
    }}
    header {{
      text-align: center;
      margin-bottom: 4rem;
    }}
    h1 {{
      font-family: var(--font-sans);
      font-weight: 300;
      font-size: 2.25rem;
      letter-spacing: 0.05em;
      text-transform: uppercase;
      color: var(--accent);
      margin: 0 0 1rem 0;
    }}
    p {{
      font-size: 1.15rem;
      margin: 1.5rem 0;
    }}
    h2, h3, h4 {{
      font-family: var(--font-sans);
      font-weight: 600;
      letter-spacing: 0.02em;
      margin-top: 2.5rem;
    }}
    img {{
      display: block;
      max-width: 100%;
      height: auto;
      margin: 3rem auto;
      border-radius: 4px;
      box-shadow: 0 4px 20px rgba(0,0,0,0.15);
    }}
    .photo-row {{
      display: flex;
      justify-content: center;
      gap: 1.5rem;
      margin: 3rem 0;
      flex-wrap: wrap;
    }}
    .photo-row img {{
      margin: 0;
      flex: 1 1 200px;
      object-fit: contain;
      max-height: 500px;
    }}
    @media print {{
      body {{
        background: #fff !important;
        color: #000 !important;
        padding: 0;
      }}
      main {{
        max-width: 100%;
      }}
      img {{
        page-break-inside: avoid;
        box-shadow: none;
      }}
    }}
  </style>
</head>
<body>
  <main>
    <header>
      <h1>{title}</h1>
    </header>
    <article>
      {body_html}
    </article>
  </main>
</body>
</html>"#, title = title, body_html = body_html);
        std::fs::write(destp.join("index.html"), html_content).map_err(|e| e.to_string())?;
        emit(total, "", "note");
        Ok(())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Publish the folder's story to the Garden: develop each marked photo
/// (2048 px JPEG beside the RAW), upload attachments (content-addressed
/// dedup), rewrite embeds, PUT the note. `dry_run` stops before any write
/// to the server (existence checks only). Emits `publish-progress`.
#[tauri::command]
async fn publish_story(
    app: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
    dir: String,
    dry_run: bool,
) -> Result<String, String> {
    let engine = state.0.clone();
    let client = garden_client(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let dirp = std::path::Path::new(&dir);
        let stems = story::stems(dirp);
        if stems.is_empty() {
            return Err("no photos in the story".to_string());
        }
        let total = stems.len();
        let emit = |done: usize, current: &str, phase: &str| {
            let _ = app.emit(
                "publish-progress",
                serde_json::json!({ "done": done, "total": total, "current": current, "phase": phase }),
            );
        };

        // 1. resolve each marked photo to <stem>.jpg beside the RAW
        let mut urls: Vec<(String, String)> = Vec::new();
        for (i, stem) in stems.iter().enumerate() {
            emit(i, stem, "preparing");

            // Look for pre-developed preview sidecar first (<stem>.preview.jpg, <stem>.jpg, etc.)
            let candidates = [
                dirp.join(format!("{stem}.preview.jpg")),
                dirp.join(format!("{stem}.jpg")),
                dirp.join(format!("{stem}.JPG")),
                dirp.join(format!("{stem}.jpeg")),
            ];

            let mut existing_jpeg: Option<Vec<u8>> = candidates.iter().find_map(|p| {
                if p.exists() {
                    std::fs::read(p).ok().filter(|b| !b.is_empty())
                } else {
                    None
                }
            });

            let raw_opt = reveal_decode::RAW_EXTENSIONS
                .iter()
                .map(|e| dirp.join(format!("{stem}.{e}")))
                .chain(
                    reveal_decode::RAW_EXTENSIONS
                        .iter()
                        .map(|e| dirp.join(format!("{stem}.{}", e.to_uppercase()))),
                )
                .find(|p| p.exists());

            let recipe = raw_opt.as_ref().and_then(|raw| {
                reveal_meta::read(raw)
                    .ok()
                    .flatten()
                    .and_then(|s| s.engine_settings)
                    .and_then(|v| serde_json::from_value(v).ok())
            }).unwrap_or_default();

            if existing_jpeg.is_none() {
                if let Some(raw) = &raw_opt {
                    if let Ok(cache_p) = developed_preview_cache_path(&app, raw, 2048, served_preview_mtime(raw)) {
                        if cache_p.exists() {
                            existing_jpeg = std::fs::read(&cache_p).ok().filter(|b| !b.is_empty());
                        }
                    }
                }
            }

            let jpeg = match existing_jpeg {
                Some(bytes) => bytes,
                None => {
                    let raw = raw_opt.ok_or_else(|| format!("RAW not found for {stem}"))?;
                    emit(i, stem, "developing");
                    let (bytes, _, _) = engine
                        .export_jpeg(&raw, &recipe, 2048, 0.0)
                        .map_err(|e| format!("develop {stem}: {e:#}"))?;
                    bytes
                }
            };

            let jpg_path = dirp.join(format!("{stem}.jpg"));
            let should_write = match std::fs::read(&jpg_path) {
                Ok(existing) => existing != jpeg,
                Err(_) => true,
            };
            if should_write {
                let _ = std::fs::write(&jpg_path, &jpeg);
            }

            // 2. upload (dedup) — dry run stops at the existence check
            emit(i, stem, if dry_run { "verifying" } else { "uploading" });
            if dry_run {
                urls.push((stem.clone(), format!("dry-run://{stem}.jpg")));
            } else {
                let url = client
                    .ensure_attachment(&jpeg, &format!("{stem}.jpg"))
                    .map_err(|e| e.to_string())?;
                urls.push((stem.clone(), url));
            }
        }

        // 3. the note
        let name = dirp.file_name().unwrap_or_default().to_string_lossy().to_string();
        let slug = reveal_publish::slugify(&name);
        let content = story::content_for_publish(dirp, &urls).map_err(|e| e.to_string())?;
        emit(total, &name, "note");
        if dry_run {
            eprintln!("publish dry-run: slug={slug}, {} photos, note ok", urls.len());
            return Ok(format!("dry-run — slug {slug}, {} photos ready", urls.len()));
        }
        let live = client.put_note(&slug, &name, &content).map_err(|e| e.to_string())?;
        let _ = story::stamp_garden_url(dirp, &live);
        eprintln!("publié: {live}");
        Ok(live)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// PUBLIER, one frame — the Swift `publishSelectedPhotoToGarden`: develop
/// with the photo's own recipe, upload (content-addressed, dedup'd), PUT a
/// one-image note. Same credentials and slug rule as the story path.
#[tauri::command]
async fn publish_photo(
    app: tauri::AppHandle,
    state: tauri::State<'_, EngineState>,
    path: String,
    allow_download: bool,
) -> Result<String, String> {
    let engine = state.0.clone();
    let client = garden_client(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let raw = std::path::PathBuf::from(&path);
        let stem = raw
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let emit = |phase: &str| {
            let _ = app.emit(
                "publish-progress",
                serde_json::json!({ "done": 0, "total": 1, "current": stem, "phase": phase }),
            );
        };

        emit("developing");
        let sidecar = reveal_meta::read(&apple_photos::metadata_path(&path)?).map_err(|e| e.to_string())?;
        let recipe = sidecar
            .as_ref()
            .and_then(|s| s.engine_settings.clone())
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let (jpeg, _, _) = engine
            .export_jpeg(&apple_photos::source(&path)?, &recipe, 2048, 0.0)
            .map_err(|e| format!("develop {stem}: {e:#}"))?;

        emit("uploading");
        let url = client
            .ensure_attachment(&jpeg, &format!("{stem}.jpg"))
            .map_err(|e| e.to_string())?;

        emit("note");
        let slug = if apple_photos::is_asset(&path) {
            format!("{}-{:016x}", reveal_publish::slugify(&stem), developed_preview_source_key(&raw))
        } else {
            reveal_publish::slugify(&stem)
        };
        let caption = sidecar.and_then(|s| s.description).unwrap_or_default();
        let now = chrono::Utc::now().to_rfc3339();
        let content = format!(
            "---\npublish: true\ntype: gallery\ntheme: gallery\ndownload: {allow_download}\ntags:\n  - gallery\n  - photo\ncreated: {now}\nmodified: {now}\n---\n\n![]({url})\n\n{caption}\n"
        );
        client
            .put_note(&slug, &stem, &content)
            .map_err(|e| e.to_string())?;
        let live = client.live_url(&slug);
        eprintln!("publié: {live}");
        Ok(live)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Caption (dc:description) editing — sidecar field, everything else kept.
#[tauri::command]
async fn save_caption(path: String, description: String) -> Result<(), String> {
    apple_photos::update_metadata(&path, |sidecar| {
        sidecar.description = if description.trim().is_empty() { None } else { Some(description) };
        Ok(())
    })
}

/// Tags (dc:subject) editing — sidecar field, everything else kept.
#[tauri::command]
async fn save_tags(path: String, tags: Vec<String>) -> Result<(), String> {
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
async fn generate_tags(app: tauri::AppHandle, path: String) -> Result<Vec<String>, String> {
    let (_, _, _, api_key, model, provider) = read_ai_cull_prefs(&app);
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

/// Cards (removable volumes with a DCIM of RAWs) currently mounted.
#[tauri::command]
async fn find_cards() -> Vec<reveal_import::Card> {
    reveal_import::find_cards()
}

/// Ingest a card's DCIM into the archive's dated layout. Emits
/// `import-progress` {done, total, current} along the way.
#[tauri::command]
async fn import_card(
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
fn cancel_import(cancel_state: tauri::State<'_, ImportCancelState>) {
    cancel_state.0.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Eject the card's volume after an import — the last step of the ingest
/// loop so the user can just pull the card out. `volume` is the mount point
/// (e.g. `/Volumes/X100F`); `diskutil eject` unmounts and powers it down.
#[tauri::command]
async fn eject_card(volume: String) -> Result<(), String> {
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

/// Export one photo (its saved recipe unless one is passed) to `dest_dir`.
#[tauri::command]
async fn export_photo(
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
async fn export_to_daily_note(
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
async fn export_batch_to_daily_note(
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
fn write_photo_export(
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
fn export_batch(
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
async fn export_photos(
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
fn cancel_exports(cancellation: tauri::State<'_, ExportState>) {
    cancellation
        .0
        .store(true, std::sync::atomic::Ordering::Release);
}

/// The generic preferences bag (`prefs.json`, the same file
/// `load_preferences`/`save_preferences` read/write for the Settings modal)
/// — reused here rather than adding dedicated `ShellPrefs` fields + setter
/// commands for four AI-cull settings.
/// (mark_story, export_desktop, target, api_key, model) — the two outcomes
/// are independent toggles now, not one master switch: a walk-away run can
/// mark picks into the story, export JPEGs to the Desktop, or both.
fn read_ai_cull_prefs(
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
struct CullResult {
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
async fn ai_cull(
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
fn cancel_cull(cancel_state: tauri::State<'_, CullCancelState>) {
    cancel_state.0.store(true, std::sync::atomic::Ordering::Relaxed);
}

#[derive(Clone, serde::Serialize)]
struct CullSelectionResult {
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
async fn ai_cull_selection(
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

/// The engine's default recipe — single source of truth for the panel.
#[tauri::command]
fn default_recipe() -> reveal_engine::Recipe {
    reveal_engine::Recipe::default()
}

/// The dev panel's INFO spec sheet — EXIF straight off the RAW's metadata
/// block (no pixel decode, so it's cheap even over the NFS mount).
#[derive(serde::Serialize)]
struct ExifInfo {
    aperture: Option<f32>,
    shutter: Option<String>,
    iso: Option<u32>,
    focal_mm: Option<f32>,
    captured_at: Option<String>,
    make: String,
    model: String,
    width: Option<u32>,
    height: Option<u32>,
}

#[tauri::command]
async fn frame_info(path: String) -> Result<ExifInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if apple_photos::is_asset(&path) {
            let asset = apple_photos::info(&path)?;
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
fn list_engines(
    state: tauri::State<'_, EngineState>,
) -> Vec<reveal_engine::EngineInfo> {
    state.0.list_engines()
}

/// Decode a photo into the engine's cache without rendering it, so stepping
/// to it is instant. The frontend calls this for the neighbours of whatever
/// is open: in a cull you almost always go to the next frame, and on a
/// NAS-hosted library that step costs ~3s of network read plus ~1s of decode
/// — paid while you're still looking at the current photo instead of after
/// you've asked for the next one.
///
/// Fire-and-forget: it takes no lock the foreground render needs (see
/// pipeline_input) and any failure just means the real open pays what it
/// would have paid anyway.
#[tauri::command]
async fn prefetch_photo(state: tauri::State<'_, EngineState>, path: String) -> Result<(), String> {
    let engine = state.0.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Ok(source) = apple_photos::source(&path) {
            engine.prefetch(&source, 2048);
        }
    })
    .await
    .map_err(|e| e.to_string())
}

/// Whether the Rapid engine's per-pixel pass can run on the GPU here.
/// The frontend uses it to decide whether a live slider drag can render at
/// full preview resolution or still needs the low-res proxy: Rapid on the
/// GPU is ~24ms for 2048px against ~236ms on the CPU.
///
/// Spektra is deliberately NOT covered by this. It already runs on the GPU
/// (spektrafilm-gpu picks its own wgpu backend) and is still ~2.5s a frame
/// — that's the spectral simulation's own cost, so it keeps the proxy no
/// matter what this returns.
#[tauri::command]
fn gpu_available() -> bool {
    reveal_engine::rapid_gpu::available()
}

/// Film and paper stocks available to the pickers.
#[tauri::command]
fn list_profiles(
    state: tauri::State<'_, EngineState>,
) -> Result<Vec<reveal_engine::ProfileEntry>, String> {
    state.0.list_profiles().map_err(|e| format!("{e:#}"))
}

/// User `.cube` LUTs available to the LUT-stack pickers (dev panel).
#[tauri::command]
fn list_luts(state: tauri::State<'_, EngineState>) -> Result<Vec<String>, String> {
    state.0.list_luts().map_err(|e| format!("{e:#}"))
}

/// Where the user drops their own `.cube` files — the LUTs section's
/// "Révéler dans le Finder" opens exactly this path via `open_path`.
#[tauri::command]
fn luts_dir(state: tauri::State<'_, EngineState>) -> String {
    state.0.luts_dir().to_string_lossy().into_owned()
}

// ---- Named develop presets (the Preset palette) -----------------------------
// A preset is a serialized `Recipe` stored under <app_data>/presets/. Save the
// current look once, apply it to any photo(s) later.

fn presets_dir_for(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(preset::presets_dir(
        &app.path().app_data_dir().map_err(|e| e.to_string())?,
    ))
}

#[tauri::command]
fn list_presets(app: tauri::AppHandle) -> Result<Vec<preset::PresetEntry>, String> {
    Ok(preset::list(&presets_dir_for(&app)?))
}

#[tauri::command]
fn save_preset(
    app: tauri::AppHandle,
    name: String,
    recipe: reveal_engine::Recipe,
) -> Result<(), String> {
    preset::save(&presets_dir_for(&app)?, &name, &recipe)
}

#[tauri::command]
fn delete_preset(app: tauri::AppHandle, name: String) -> Result<(), String> {
    preset::delete(&presets_dir_for(&app)?, &name)
}

/// Import Lightroom / Camera Raw `.xmp` presets as Reveal presets. Opens a
/// multi-select picker; each file that converts is saved under its own name
/// from the XMP. Returns one report per file so the UI can say what came
/// across and what Reveal has no equivalent for (see xmp_preset.rs) — a
/// preset built mostly out of masks and lens profiles can import "fine" and
/// still look nothing like it did in Lightroom.
#[tauri::command]
async fn import_xmp_presets(app: tauri::AppHandle) -> Result<Vec<xmp_preset::ImportReport>, String> {
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
        match xmp_preset::parse(&text) {
            Ok((recipe, report)) => {
                if let Err(e) = preset::save(&dir, &report.name, &recipe) {
                    errors.push(format!("{label}: {e}"));
                } else {
                    reports.push(report);
                }
            }
            Err(e) => errors.push(format!("{label}: {e}")),
        }
    }

    // Some files failing shouldn't discard the ones that worked — only a
    // run where NOTHING imported is worth surfacing as an error.
    if reports.is_empty() && !errors.is_empty() {
        return Err(errors.join("\n"));
    }
    Ok(reports)
}

/// Read the photo's sidecar (rating, tags, saved recipe). Null when none.
#[tauri::command]
async fn load_sidecar(path: String) -> Result<Option<reveal_meta::Sidecar>, String> {
    reveal_meta::read(&apple_photos::metadata_path(&path)?).map_err(|e| e.to_string())
}

/// Persist the recipe into the photo's sidecar, preserving the standard
/// fields (rating, caption, tags) already there. Shared with the
/// default-import-preset path in `import_card` — same write, same
/// "preserve whatever's already in the sidecar" behavior.
fn write_recipe_to_sidecar(path: &std::path::Path, recipe: &reveal_engine::Recipe) -> Result<(), String> {
    apple_photos::update_metadata(&path.to_string_lossy(), |sidecar| {
        sidecar.engine = Some(recipe.engine.clone());
        sidecar.engine_settings = Some(serde_json::to_value(recipe).map_err(|e| e.to_string())?);
        Ok(())
    })
}

#[tauri::command]
async fn save_recipe(path: String, recipe: reveal_engine::Recipe) -> Result<(), String> {
    write_recipe_to_sidecar(std::path::Path::new(&path), &recipe)
}

#[tauri::command]
async fn clear_recipe(path: String) -> Result<(), String> {
    let metadata = apple_photos::metadata_path(&path)?;
    let p = metadata.as_path();
    apple_photos::update_metadata(&path, |sidecar| {
        sidecar.engine = None;
        sidecar.engine_settings = None;
        Ok(())
    })?;
    // Reverting to "no engine" removes our own developed sidecar so the grid
    // and loupe fall back to the as-shot look. The Swift-era `.reveal.jpg` is
    // left untouched (manual cleanup later) — it stays a read-only fallback.
    if let Some(candidate) = preview_sidecar_path(p) {
        match std::fs::remove_file(&candidate) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

/// Per-frame develop-sidecar mtimes (ms since epoch, 0 = as-shot), parallel to
/// `paths`. The grid feeds these back as thumb cache-busting versions so an
/// external edit to a `.preview.jpg` shows up on the next load — file over app.
#[tauri::command]
async fn preview_versions(paths: Vec<String>) -> Result<Vec<u64>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        paths
            .par_iter()
            .map(|p| served_preview_mtime(std::path::Path::new(p)))
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}

struct EngineState(std::sync::Arc<reveal_engine::Engine>);

/// spektrafilm data dir: bundled resource in the .app, the repo copy in dev.
fn resolve_data_dir(app: &tauri::App) -> std::path::PathBuf {
    if let Ok(dir) = app.path().resource_dir() {
        let candidate = dir.join("data");
        if candidate.join("profiles").is_dir() {
            return candidate;
        }
    }
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data")
}

/// Minimal percent-decoding (enough for file paths from
/// `encodeURIComponent`).
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Protocol smoke test — proves the webview can load pixels straight from
/// Rust through `reveal://`. Real handlers (thumb/proxy/preview) land in M2+.
const PING_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 240 150">
  <rect width="240" height="150" fill="#1f1f1e"/>
  <rect x="10" y="10" width="220" height="130" fill="none" stroke="#d6202c" stroke-width="2"/>
  <text x="120" y="70" fill="#ebebeb" font-family="monospace" font-size="16" text-anchor="middle">reveal://ping</text>
  <text x="120" y="95" fill="#d6202c" font-family="monospace" font-size="12" text-anchor="middle">served from Rust</text>
</svg>"##;

fn is_volume_mounted(path: &std::path::Path) -> bool {
    if let Ok(strip) = path.strip_prefix("/Volumes") {
        if let Some(vol_name) = strip.components().next() {
            let vol_path = std::path::Path::new("/Volumes").join(vol_name);
            if !vol_path.is_dir() {
                return false;
            }
        }
    }
    true
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app, shortcut, event| {
            use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};
            if event.state() == ShortcutState::Pressed {
                if shortcut.mods.contains(Modifiers::ALT) && shortcut.key == Code::KeyR {
                    toggle_window_visibility(app);
                }
            }
        }).build())
        .plugin(tauri_plugin_window_state::Builder::default().with_denylist(&["import-panel"]).build())
        .setup(|app| {
            apple_photos::init(app.handle())?;
            setup_main_menu(app).map_err(|e| e.to_string())?;
            setup_tray(app).map_err(|e| e.to_string())?;

            // The browser sign-in flow (standard.garden/connect/reveal) hands a
            // freshly-minted key back via `reveal://garden-callback?key=...` —
            // the Tauri counterpart of Swift's `application(_:open:)` (which
            // this app now replaces — same scheme, one app owns it).
            // Scheme is registered at bundle time via tauri.conf.json's
            // `plugins.deep-link.desktop.schemes` (baked into Info.plist), so
            // nothing to call at runtime on macOS.
            {
                use tauri_plugin_deep_link::DeepLinkExt;
                let dl_app = app.handle().clone();
                app.deep_link().on_open_url(move |event| {
                    for url in event.urls() {
                        if url.scheme() != "reveal" || url.host_str() != Some("garden-callback") {
                            continue;
                        }
                        let Some(key) = url
                            .query_pairs()
                            .find(|(k, _)| k == "key")
                            .map(|(_, v)| v.into_owned())
                        else {
                            continue;
                        };
                        let app_for_task = dl_app.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = sign_in_with_key(&app_for_task, &key).await {
                                let _ = app_for_task.emit(
                                    "app-error",
                                    serde_json::json!({ "message": format!("connexion Garden: {e}") }),
                                );
                            }
                        });
                    }
                });
            }

            // Register global hotkey Option+R (⌥R)
            use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
            let shortcut = Shortcut::new(Some(Modifiers::ALT), Code::KeyR);
            let _ = app.global_shortcut().register(shortcut);

            // Silent autostart check
            let args: Vec<String> = std::env::args().collect();
            let at_login = args.contains(&"--at-login".to_string());
            if !at_login {
                show_main_window(app.handle());
            }

            let shell_prefs = read_shell_prefs(app.handle());
            app.manage(FocusState(std::sync::Arc::new(std::sync::Mutex::new(shell_prefs.focus_mode))));
            app.manage(FocusPresenceState::default());
            app.manage(OpenFileState::default());
            app.manage(ImportState::default());
            app.manage(ImportCancelState::default());
            app.manage(ExportState::default());
            app.manage(CullState::default());
            app.manage(CullCancelState::default());
            app.manage(ThumbConcurrencyState(std::sync::Arc::new(ThumbSemaphore::new(6))));
            #[cfg(target_os = "macos")]
            macos::volume_watcher::start(app.handle().clone());

            #[cfg(not(target_os = "macos"))]
            start_card_watcher(app.handle().clone());

            let data_dir = resolve_data_dir(app);
            let luts_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("app_data_dir: {e}"))?
                .join("luts");
            let engine = reveal_engine::Engine::new(&data_dir, &luts_dir)
                .map_err(|e| format!("engine init ({}): {e:#}", data_dir.display()))?;
            eprintln!("engine ready — backend: {}", engine.backend_name());
            // Where a Develop session parks its decoded frame so a restart
            // skips the NAS read and the decode entirely.
            if let Ok(cache) = app.handle().path().app_cache_dir() {
                engine.set_working_dir(Some(cache.join("WorkingFrame")));
            }
            app.manage(EngineState(std::sync::Arc::new(engine)));

            let db = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("app_data_dir: {e}"))?
                .join("index-rs.sqlite");
            let index = reveal_index::Index::open(&db)
                .map_err(|e| format!("index ({}): {e}", db.display()))?;
            app.manage(IndexState(std::sync::Arc::new(index)));
            // Bound the develop cache on launch, in case a long session left it
            // over the limit.
            schedule_cache_prune(app.handle());
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("reveal", |_ctx, request, responder| {
            // reveal://<host>?… — host is the verb on macOS WKWebView.
            let host = request.uri().host().unwrap_or_default().to_string();
            match host.as_str() {
                "ping" => {
                    let response = HttpResponse::builder()
                        .header("Content-Type", "image/svg+xml")
                        .body(PING_SVG.as_bytes().to_vec())
                        .unwrap();
                    responder.respond(response);
                }
                // reveal://log?m=… — webview console/error bridge: WKWebView
                // has no visible console in release, so the page ships its
                // errors here and they land on stderr with everything else.
                "log" => {
                    let msg = request
                        .uri()
                        .query()
                        .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("m=")))
                        .map(percent_decode)
                        .unwrap_or_default();
                    eprintln!("js: {msg}");
                    let response = HttpResponse::builder()
                        .header("Content-Type", "text/plain")
                        .body(b"ok".to_vec())
                        .unwrap();
                    responder.respond(response);
                }

                // reveal://thumb?p=<percent-encoded path> — the camera's
                // embedded JPEG, extracted on demand (~10-30 ms). The grid's
                // lazy-loading paces the requests.
                "thumb" => {
                    let app = _ctx.app_handle().clone();
                    let size = request.uri().query()
                        .and_then(|query| query.split('&').find_map(|pair| pair.strip_prefix("size=")))
                        .and_then(|value| value.parse::<u32>().ok())
                        .unwrap_or(GRID_PREVIEW_EDGE).clamp(256, 2560);
                    // The version the frontend believes this photo's preview
                    // is at. Part of the cache key, so an edit to
                    // `.preview.jpg` outside Reveal moves the key and the
                    // stale entry is simply not found.
                    let version = request.uri().query()
                        .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("v=")))
                        .and_then(|value| value.parse::<u64>().ok())
                        .unwrap_or(0);
                    // The photo actually on screen must not queue behind the
                    // grid. Restoring a session fires ~120 cell requests and
                    // then opens one photo; on a loaded NAS a cell took 4-6s
                    // here, so the one image the photographer is waiting for
                    // sat behind all of them (Francis: "j'ai la photo, mais
                    // ça a été très long"). There is at most one of these at
                    // a time, so it skips the queue entirely.
                    let priority = request.uri().query()
                        .is_some_and(|q| q.split('&').any(|kv| kv == "priority=1"));
                    let path = request
                        .uri()
                        .query()
                        .and_then(|q| q.split('&').find_map(|kv| kv.strip_prefix("p=")))
                        .map(percent_decode)
                        .unwrap_or_default();
                    if path.is_empty() {
                        let response = HttpResponse::builder().status(404).body(Vec::new()).unwrap();
                        responder.respond(response);
                        return;
                    }
                    // The local cache before the mount guard, deliberately.
                    // These bytes are on this disk; whether the NAS is awake
                    // is beside the point. Guarding first meant that on a
                    // cold start — NFS automount not yet materialised — the
                    // photo you were editing came back "Preview unavailable"
                    // while its pixels sat in the cache, and only a trip
                    // through the grid (which woke the mount) fixed it
                    // (Francis, 2026-09-22).
                    if let Ok(local) = developed_preview_cache_path(
                        &app,
                        std::path::Path::new(&path),
                        size,
                        version,
                    ) {
                        if let Ok(bytes) = std::fs::read(&local) {
                            eprintln!(
                                "thumb: {} (local cache, {} ko)",
                                path.rsplit('/').next().unwrap_or(&path),
                                bytes.len() / 1024
                            );
                            responder.respond(
                                HttpResponse::builder()
                                    .header("Content-Type", "image/jpeg")
                                    .header("Cache-Control", "max-age=3600")
                                    .body(bytes)
                                    .unwrap(),
                            );
                            return;
                        }
                    }
                    if !is_volume_mounted(std::path::Path::new(&path)) {
                        // Offline. The exact key above needed a version the
                        // NAS alone can tell us, so fall back to the newest
                        // render this disk holds for the photo — there is no
                        // fresher truth available to compare it against.
                        // Tell the UI the source is unreachable. Whether a
                        // cached copy answers below or not, the photographer
                        // should know they are looking at what this machine
                        // remembers rather than at the archive.
                        let _ = app.emit("source-offline", serde_json::json!({ "path": path }));
                        let local = newest_cached_render(
                            &app,
                            std::path::Path::new(&path),
                            size,
                        )
                        .and_then(|p| std::fs::read(p).ok());
                        let response = match local {
                            Some(bytes) => {
                                eprintln!(
                                    "thumb: {} (offline — newest local copy, {} ko)",
                                    path.rsplit('/').next().unwrap_or(&path),
                                    bytes.len() / 1024
                                );
                                HttpResponse::builder()
                                    .header("Content-Type", "image/jpeg")
                                    .header("Cache-Control", "max-age=3600")
                                    .body(bytes)
                                    .unwrap()
                            }
                            None => HttpResponse::builder().status(404).body(Vec::new()).unwrap(),
                        };
                        responder.respond(response);
                        return;
                    }
                    let sem = app.state::<ThumbConcurrencyState>().0.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        // Bounds how many of these run at once — see
                        // `ThumbSemaphore`'s doc comment for why this exists.
                        // A priority request holds no permit: it is the photo
                        // on screen, and there is only ever one.
                        let _permit = (!priority).then(|| ThumbPermit::acquire(&sem));
                        if apple_photos::is_asset(&path) {
                            let response = match apple_photos::thumbnail(&path, size) {
                                Ok(bytes) => HttpResponse::builder()
                                    .header("Content-Type", "image/jpeg")
                                    .header("Cache-Control", "no-cache")
                                    .body(if size <= 768 { downscale_grid_thumb(bytes, size) } else { bytes }).unwrap(),
                                Err(error) => {
                                    eprintln!("Apple Photos thumbnail: {error}");
                                    HttpResponse::builder().status(503).body(error.into_bytes()).unwrap()
                                }
                            };
                            responder.respond(response);
                            return;
                        }
                        let t = std::time::Instant::now();
                        let source = std::path::Path::new(&path);

                        // File over app: the developed `.preview.jpg` sibling of
                        // the RAW is the truth. It exists iff the photo was developed.
                        let sidecar = preview_sidecar_path(source)
                            .filter(|candidate| candidate.is_file());
                        let response = if let Some(preview_path) = sidecar {
                            match std::fs::read(&preview_path) {
                                Ok(bytes) => {
                                    eprintln!(
                                        "thumb: {} (developed sidecar, {} ko, {} ms)",
                                        source.file_name().unwrap_or_default().to_string_lossy(),
                                        bytes.len() / 1024,
                                        t.elapsed().as_millis()
                                    );
                                    // Resize once, then both serve and keep
                                    // it. Keeps the NAS round trip to once
                                    // per photo per size, and the decode and
                                    // re-encode to once rather than once per
                                    // request.
                                    let sized = downscale_grid_thumb(bytes, size);
                                    cache_developed_preview_locally(&app, source, &sized, size, version);
                                    HttpResponse::builder()
                                        .header("Content-Type", "image/jpeg")
                                        .header("Cache-Control", "max-age=3600")
                                        .body(sized)
                                        .unwrap()
                                }
                                Err(e) => {
                                    eprintln!("thumb {} legacy preview: {e}", preview_path.display());
                                    HttpResponse::builder().status(404).body(Vec::new()).unwrap()
                                }
                            }
                        } else {
                            // The camera's embedded preview is cheapest and
                            // tried FIRST — a grid cell only needs
                            // `GRID_THUMB_MAX_EDGE` px, so decoding a
                            // full-resolution companion JPEG here (14-26 MB
                            // compressed, 60-100+ MB once decoded) was pure
                            // waste for the common case, and with no
                            // concurrency limit on this handler it could spike
                            // memory into the tens of GB opening one RAW+JPEG
                            // folder (confirmed 2026-08-02, see reveal.md). The
                            // companion JPEG — and full develop — now only run
                            // when there's no embedded thumb to serve.
                            match reveal_decode::extract_thumb_preview(source) {
                            Ok(preview) => {
                                eprintln!(
                                    "thumb: {} ({}, {} ko, {} ms)",
                                    path.rsplit('/').next().unwrap_or(&path),
                                    preview.mime,
                                    preview.bytes.len() / 1024,
                                    t.elapsed().as_millis()
                                );
                                let small = downscale_grid_thumb(preview.bytes, size);
                                persist_thumb_cache(source, &small, size);
                                HttpResponse::builder()
                                    .header("Content-Type", "image/jpeg")
                                    .header("Cache-Control", "max-age=3600")
                                    .body(small)
                                    .unwrap()
                            }
                            Err(thumb_err) => if let Some(companion) = companion_jpeg_path(source) {
                                // RAW+JPEG shooting, but this RAW had no
                                // embedded thumb to fall back on cheaply: the
                                // camera wrote its own full JPEG right next to
                                // the RAW — the most faithful "as shot" source,
                                // correct even for a monochrome film
                                // simulation the sensor data alone can't
                                // reproduce (RAW is always color).
                                eprintln!("thumb {path}: no embedded preview ({thumb_err}) - trying companion jpg");
                                match std::fs::read(&companion) {
                                    Ok(bytes) => {
                                        eprintln!(
                                            "thumb: {} (companion jpg, {} ko, {} ms)",
                                            source.file_name().unwrap_or_default().to_string_lossy(),
                                            bytes.len() / 1024,
                                            t.elapsed().as_millis()
                                        );
                                        let small = downscale_grid_thumb(bytes, size);
                                        persist_thumb_cache(source, &small, size);
                                        HttpResponse::builder()
                                            .header("Content-Type", "image/jpeg")
                                            .header("Cache-Control", "max-age=3600")
                                            .body(small)
                                            .unwrap()
                                    }
                                    Err(e) => {
                                        eprintln!("thumb {} companion jpg read: {e}", companion.display());
                                        HttpResponse::builder().status(404).body(Vec::new()).unwrap()
                                    }
                                }
                            } else {
                                // No camera-embedded JPEG (some RAWs lack one, or
                                // extraction failed) and no engine picked yet
                                // (engine=None, or we wouldn't be in this "no
                                // sidecar" branch). Render a NEUTRAL fallback —
                                // Rapid at its defaults (contrast/saturation/
                                // temperature/tint all 0, the corrected AgX
                                // curve) — and PERSIST it as `.preview.jpg`, so
                                // this is a one-time cost, not a per-thumb-request
                                // render. This only touches the durable JPEG
                                // file, never the recipe sidecar metadata: the
                                // photo still reads as "None" if reopened in dev.
                                eprintln!("thumb {path}: {thumb_err} - generating neutral fallback preview");
                                let engine = app.state::<EngineState>().0.clone();
                                let neutral = reveal_engine::Recipe {
                                    engine: "rapid".to_string(),
                                    ..reveal_engine::Recipe::default()
                                };
                                match engine.develop_jpeg(std::path::Path::new(&path), &neutral, 2048) {
                                    Ok(out) => {
                                        eprintln!(
                                            "thumb-fallback: {} ({} ko, {} ms)",
                                            path.rsplit('/').next().unwrap_or(&path),
                                            out.jpeg.len() / 1024,
                                            t.elapsed().as_millis()
                                        );
                                        persist_thumb_cache(source, &out.jpeg, DURABLE_PREVIEW_EDGE);
                                        HttpResponse::builder()
                                            .header("Content-Type", "image/jpeg")
                                            .header("Cache-Control", "max-age=3600")
                                            .body(downscale_grid_thumb(out.jpeg, size))
                                            .unwrap()
                                    }
                                    Err(dev_e) => {
                                        // Last resort: the file may not be a
                                        // RAW at all. A Google Takeout export
                                        // hands back JPEGs still named `.DNG`
                                        // — 113 of them in one folder here,
                                        // 4032x3024, that libraw cannot touch
                                        // and every branch above therefore
                                        // refuses. Read the bytes as an
                                        // ordinary image before giving up.
                                        match plain_image_bytes(std::path::Path::new(&path)) {
                                            Some(bytes) => {
                                                eprintln!(
                                                    "thumb: {} (not a raw — plain image, {} ko)",
                                                    path.rsplit('/').next().unwrap_or(&path),
                                                    bytes.len() / 1024
                                                );
                                                HttpResponse::builder()
                                                    .header("Content-Type", "image/jpeg")
                                                    .header("Cache-Control", "max-age=3600")
                                                    .body(downscale_grid_thumb(bytes, size))
                                                    .unwrap()
                                            }
                                            None => {
                                                eprintln!("thumb fallback {path}: {dev_e:#}");
                                                HttpResponse::builder().status(404).body(Vec::new()).unwrap()
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        };
                        responder.respond(response);
                    });
                }
                _ => {
                    let response = HttpResponse::builder().status(404).body(Vec::new()).unwrap();
                    responder.respond(response);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            apple_photos::apple_photos_status,
            apple_photos::apple_photos_albums,
            apple_photos::apple_photos_list,
            apple_photos::apple_photos_cancel,
            apple_photos::apple_photos_cache_status,
            apple_photos::apple_photos_cache_clear,
            developed_preview_cache_status,
            developed_preview_cache_clear,
            load_shell_prefs,
            load_preferences,
            save_preferences,
            set_auto_import,
            toggle_auto_import,
            set_import_dir,
            set_default_import_preset,
            notify_user,
            open_path,
            reveal_in_finder,
            toggle_system_appearance,
            toggle_focus,
            set_focus,
            set_focus_window_presence,
            set_simple_fullscreen,
            focus_state,
            show_contact_sheet,
            hide_contact_sheet,
            ping,
            pick_raw,
            pick_folder,
            list_dir,
            set_rating,
            scan_root,
            scan_folder,
            add_catalog_root,
            catalog_roots,
            source_reachable,
            park_working_frame,
            release_working_frame,
            remove_catalog_root,
            index_dirs,
            story_dirs,
            index_frames,
            find_cards,
            import_card,
            cancel_import,
            eject_card,
            move_photo,
            rename_dir,
            create_dir,
            move_dir,
            export_photo,
            export_to_daily_note,
            export_batch_to_daily_note,
            vault_attachment_dir,
            export_photos,
            cancel_exports,
            ai_cull,
            ai_cull_selection,
            cancel_cull,
            story_stems,
            story_toggle,
            publish_story,
            publish_photo,
            garden_sign_in,
            garden_sign_out,
            garden_refresh,
            load_story_note,
            save_story_note,
            story_load_theme,
            story_set_theme,
            story_set_pinned,
            list_story_notes,
            export_local_story,
            save_caption,
            save_tags,
            generate_tags,
            develop_preview,
            copy_developed_preview_to_clipboard,
            copy_photo_preview_to_clipboard,
            develop_preview_rgba,
            default_recipe,
            frame_info,
            list_engines,
            gpu_available,
            prefetch_photo,
            list_profiles,
            list_luts,
            luts_dir,
            list_presets,
            save_preset,
            delete_preset,
            import_xmp_presets,
            load_sidecar,
            save_recipe,
            clear_recipe,
            preview_versions,
            autoload_path,
            take_open_file,
            list_external_editors,
            open_in_editor,
            load_catalog_note,
            save_catalog_note
        ])
        .on_window_event(|window, event| {
            #[cfg(target_os = "macos")]
            if matches!(window.label(), "develop-panel") {
                if let Ok(ns_window) = window.ns_window() {
                    let _ = macos::develop_panel::configure(
                        ns_window as *mut objc::runtime::Object,
                    );
                }
            } else if matches!(
                event,
                tauri::WindowEvent::Resized(_)
                    | tauri::WindowEvent::ScaleFactorChanged { .. }
            ) {
                if let Ok(ns_window) = window.ns_window() {
                    let _ =
                        macos::traffic_lights::style(ns_window as *mut objc::runtime::Object);
                }
            }
            #[cfg(target_os = "macos")]
            if matches!(event, tauri::WindowEvent::Focused(true)) {
                let focus_enabled = window
                    .app_handle()
                    .try_state::<FocusState>()
                    .map(|state| *state.0.lock().unwrap())
                    .unwrap_or(false);
                if focus_enabled {
                    let _ = apply_focus_backdrop(&window.app_handle(), true);
                }
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                #[cfg(target_os = "macos")]
                if matches!(window.label(), "main") {
                    macos::focus_backdrop::hide();
                }
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Reveal")
        .run(|app, event| {
            if let tauri::RunEvent::Opened { ref urls } = event {
                if let Some(path) = urls.into_iter().find_map(|url| {
                    let path = url.to_file_path().ok()?;
                    let is_jpeg = matches!(
                        path.extension().and_then(|ext| ext.to_str()).map(str::to_ascii_lowercase).as_deref(),
                        Some("jpg" | "jpeg")
                    );
                    is_jpeg.then(|| path.to_string_lossy().into_owned())
                }) {
                    if let Some(state) = app.try_state::<OpenFileState>() {
                        *state.0.lock().unwrap() = Some(path.clone());
                    }
                    let _ = app.emit("open-file-requested", path);
                }
            }
            if let tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } = event
            {
                if !has_visible_windows {
                    show_main_window(app);
                    let _ = app.emit("dock-reopen-requested", ());
                }
            }
        });
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
