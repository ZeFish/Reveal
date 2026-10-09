//! Main application menu for macOS and other platforms.

use tauri::{Emitter, Manager};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use crate::window::show_main_window;

pub fn setup_main_menu(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
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
    let log_separator = PredefinedMenuItem::separator(app)?;
    let activity_logs =
        MenuItem::with_id(app, "menu-logs", "Activity Logs…", true, Some("CmdOrCtrl+Alt+L"))?;
    let devtools_item =
        MenuItem::with_id(app, "menu-devtools", "Toggle Developer Tools", true, Some("CmdOrCtrl+Alt+I"))?;
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
            &log_separator,
            &activity_logs,
            &devtools_item,
        ],
    )?;

    // Help: the manual lives on the site, and the app opens it in the browser.
    let manual = MenuItem::with_id(app, "menu-manual", "Reveal Manual", true, None::<&str>)?;
    let help_menu = Submenu::with_items(app, "Help", true, &[&manual])?;
    let menu = Menu::with_items(
        app,
        &[&app_menu, &edit_menu, &import_menu, &develop_menu, &export_menu, &window_menu, &help_menu],
    )?;
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| match event.id().as_ref() {
        "menu-manual" => {
            let _ = std::process::Command::new("/usr/bin/open")
                .arg("https://reveal.photos/manual/")
                .spawn();
        }
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
        "menu-logs" => {
            crate::log_capture::open_log_window(app);
        }
        "menu-devtools" => {
            if let Some(w) = app.get_webview_window("main") {
                if w.is_devtools_open() {
                    let _ = w.close_devtools();
                } else {
                    let _ = w.open_devtools();
                }
            }
        }
        _ => {}
    });
    Ok(())
}
