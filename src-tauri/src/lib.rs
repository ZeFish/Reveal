//! The Tauri shell: app setup, the window/tray/menu/focus machinery, the
//! `reveal://` protocol handler, and the one `generate_handler!` that lists
//! every command.
//!
//! This file used to be all 5313 lines of it — every one of the 96 commands,
//! their helpers and their tests. Nothing was mis-structured inside it; the
//! domains were already contiguous bands. They just had no names, so the only
//! way to find anything was to scroll, and edits landed in the wrong place.
//!
//! Each `mod` below is one of those bands, moved unchanged. Where a carve
//! turned up a call crossing two domains — a cull run exporting its picks, a
//! daily-note export writing into the vault, an import checking its
//! destination is writable — the dependency was already there. It is now
//! written down as a `use`.

#![allow(unexpected_cfgs)]

use rayon::prelude::*;
use tauri::http::Response as HttpResponse;
use tauri::ipc::Response as IpcResponse;
use tauri::{Emitter, Manager};

mod apple_photos;
mod catalog; // the library on disk: folders, ratings, roots, the index
mod cull; // AI-assisted culling
mod export; // exporting developed photos
mod import_cards; // ingesting memory cards
mod metadata; // captions and tags
mod preview; // every on-disk derivative of a photo
mod publishing; // stories, the vault, Garden
use preview::*; // the crate root still reaches for these by plain name
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
    if let Ok(client) = reveal_publish::GardenClient::from_vault(&publishing::vault_path(app)) {
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
    reveal_publish::GardenClient::from_vault(&publishing::vault_path(app)).map_err(|e| e.to_string())
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
                reveal_publish::GardenClient::from_vault(&publishing::vault_path(&app)).ok().map(|c| c.username.clone())
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
                        // An entry that is there but empty is not a hit. Serving
                        // it leaves the cell blank for good; dropping it lets
                        // this request fall through and the next one re-cache
                        // properly, so the nine that already exist heal
                        // themselves the first time they are asked for.
                        if let Ok(bytes) = std::fs::read(&local).and_then(|b| {
                            if b.is_empty() {
                                let _ = std::fs::remove_file(&local);
                                Err(std::io::Error::new(
                                    std::io::ErrorKind::InvalidData,
                                    "empty cache entry",
                                ))
                            } else {
                                Ok(b)
                            }
                        }) {
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
            preview::developed_preview_cache_status,
            preview::developed_preview_cache_clear,
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
            catalog::pick_folder,
            catalog::list_dir,
            catalog::set_rating,
            catalog::scan_root,
            catalog::scan_folder,
            catalog::add_catalog_root,
            catalog::catalog_roots,
            catalog::source_reachable,
            catalog::park_working_frame,
            catalog::release_working_frame,
            catalog::remove_catalog_root,
            catalog::index_dirs,
            publishing::story_dirs,
            catalog::index_frames,
            import_cards::find_cards,
            import_cards::import_card,
            import_cards::cancel_import,
            import_cards::eject_card,
            catalog::move_photo,
            catalog::rename_dir,
            catalog::create_dir,
            catalog::move_dir,
            export::export_photo,
            export::export_to_daily_note,
            export::export_batch_to_daily_note,
            publishing::vault_attachment_dir,
            export::export_photos,
            export::cancel_exports,
            cull::ai_cull,
            cull::ai_cull_selection,
            cull::cancel_cull,
            publishing::story_stems,
            publishing::story_toggle,
            publishing::publish_story,
            publishing::publish_photo,
            garden_sign_in,
            garden_sign_out,
            garden_refresh,
            publishing::load_story_note,
            publishing::save_story_note,
            publishing::story_load_theme,
            publishing::story_set_theme,
            publishing::story_set_pinned,
            publishing::list_story_notes,
            publishing::export_local_story,
            metadata::save_caption,
            metadata::save_tags,
            metadata::generate_tags,
            preview::develop_preview,
            preview::copy_developed_preview_to_clipboard,
            preview::copy_photo_preview_to_clipboard,
            preview::develop_preview_rgba,
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
mod frame_packing_tests {
    use super::*;

    /// The frontend reads four little-endian u32 and then treats EVERYTHING
    /// after byte 16 as pixels. Get the header wrong and the canvas shows a
    /// skewed image with a coloured band, not an error — so pin the layout.
    #[test]
    fn the_header_is_four_little_endian_u32_then_pixels() {
        let rgba = vec![7u8, 8, 9, 10, 11, 12, 13, 14];
        let packed = pack_developed_frame(2, 1, 24, 950, &rgba);

        assert_eq!(packed.len(), 16 + rgba.len());
        assert_eq!(u32::from_le_bytes(packed[0..4].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(packed[4..8].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(packed[8..12].try_into().unwrap()), 24);
        assert_eq!(u32::from_le_bytes(packed[12..16].try_into().unwrap()), 950);
        assert_eq!(&packed[16..], &rgba[..]);
    }

    /// A 2048px frame must cross the bridge as its own 11.2 MB and not one
    /// byte more. This is the whole reason the command stopped returning a
    /// serde struct: as JSON the same frame weighed 44.7 MB and cost 68 ms
    /// to encode in release, against ~24 ms to render it.
    #[test]
    fn a_full_frame_costs_its_pixels_plus_a_header() {
        let (w, h) = (2048u32, 1365u32);
        let rgba = vec![128u8; (w * h * 4) as usize];
        let packed = pack_developed_frame(w, h, 24, 0, &rgba);
        assert_eq!(packed.len(), 16 + (w * h * 4) as usize);
        assert!(packed.len() < 12_000_000, "{} bytes", packed.len());
    }
}
