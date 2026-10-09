//! Window management: focus mode, backdrop, traffic lights, fullscreen, visibility, and HUD panels.

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use crate::shell_prefs::{read_shell_prefs, write_shell_prefs};

#[derive(Clone, Default)]
pub struct FocusState(pub std::sync::Arc<std::sync::Mutex<bool>>);

#[derive(Clone, Default)]
pub struct FocusPresenceState(pub std::sync::Arc<std::sync::Mutex<std::collections::BTreeSet<String>>>);

#[derive(Clone, Default)]
pub struct OpenFileState(pub std::sync::Arc<std::sync::Mutex<Option<String>>>);

#[derive(Clone, Default)]
pub struct ImportState(pub std::sync::Arc<std::sync::Mutex<std::collections::BTreeSet<String>>>);

/// The import's stop flag — one import runs at a time in practice, so a
/// single shared flag (reset when an import starts) covers the HUD's stop.
#[derive(Clone, Default)]
pub struct ImportCancelState(pub std::sync::Arc<std::sync::atomic::AtomicBool>);

pub fn show_import_panel(app: &AppHandle) {
    let already_in_app = app
        .get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if already_in_app {
        return;
    }
    let app_handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(panel) = app_handle.get_webview_window("import-panel") {
            if let Ok(Some(monitor)) = app_handle.primary_monitor() {
                let scale_factor = monitor.scale_factor();
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

/// The import panel's "Ouvrir Reveal": it hid itself and asked for the main
/// window through a command that was never registered, so the click closed
/// the panel and showed nothing.
#[tauri::command]
pub fn reveal_main_window(app: AppHandle) {
    show_main_window(&app);
}

pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        #[cfg(target_os = "macos")]
        {
            if let Ok(ns_window) = window.ns_window() {
                let _ = crate::macos::traffic_lights::style(ns_window as *mut objc::runtime::Object);
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

pub fn hide_main_window(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    crate::macos::focus_backdrop::hide();

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

pub fn toggle_window_visibility(app: &AppHandle) {
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

pub fn emit_focus_mode(app: &AppHandle, enabled: bool) {
    let _ = app.emit("focus-mode-changed", serde_json::json!({ "enabled": enabled }));
}

pub fn apply_focus_mode(app: &AppHandle, enabled: bool) -> Result<(), String> {
    apply_focus_backdrop(app, enabled)?;
    emit_focus_mode(app, enabled);
    Ok(())
}

pub fn apply_focus_backdrop(app: &AppHandle, enabled: bool) -> Result<(), String> {
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
                let _ = crate::macos::focus_backdrop::show_below(ns_win);
            } else {
                crate::macos::focus_backdrop::hide();
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

pub fn toggle_macos_appearance() -> Result<(), String> {
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
        Err("This command is only available on macOS".into())
    }
}

/// Set Reveal's focus mode explicitly.
#[tauri::command]
pub fn set_focus(app: AppHandle, enabled: bool) -> Result<(), String> {
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
pub fn set_focus_window_presence(
    app: AppHandle,
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
        std::thread::sleep(std::time::Duration::from_millis(100));
        let still_outside = delayed_app
            .try_state::<FocusPresenceState>()
            .map(|state| state.0.lock().unwrap().is_empty())
            .unwrap_or(true);
        let enabled = delayed_app
            .try_state::<FocusState>()
            .map(|state| *state.0.lock().unwrap())
            .unwrap_or(false);
        if still_outside && enabled {
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

/// Borderless fullscreen for the main window.
#[tauri::command]
pub fn set_simple_fullscreen(window: WebviewWindow, enabled: bool) -> Result<(), String> {
    window.set_fullscreen(enabled).map_err(|e| e.to_string())
}

/// Hide native macOS traffic light buttons for a window so that custom
/// HTML controls can render in their place with exact hover behaviors.
#[tauri::command]
pub fn hide_window_traffic_lights(window: WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let ns_window = window.ns_window().map_err(|e| e.to_string())? as *mut objc::runtime::Object;
        crate::macos::traffic_lights::style(ns_window)?;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = &window;
    }
    Ok(())
}

/// Hide the main Reveal window while keeping the app alive.
#[tauri::command]
pub fn hide_contact_sheet(app: AppHandle) {
    hide_main_window(&app);
}

/// Toggle the system-wide macOS light/dark appearance.
#[tauri::command]
pub fn toggle_system_appearance() -> Result<(), String> {
    toggle_macos_appearance()
}

#[tauri::command]
pub fn take_open_file(state: tauri::State<'_, OpenFileState>) -> Option<String> {
    state.0.lock().unwrap().take()
}
