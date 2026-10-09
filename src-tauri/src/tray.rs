//! System tray menu and icon management.

use tauri::{Emitter, Manager};
use tauri::menu::{CheckMenuItemBuilder, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use crate::shell_prefs::read_shell_prefs;
use crate::window::show_main_window;

pub struct TrayMenuState {
    pub auto_import_item: tauri::menu::CheckMenuItem<tauri::Wry>,
}

pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let shell_prefs = read_shell_prefs(app.handle());

    let show = MenuItem::with_id(app, "show", "Open Reveal", true, None::<&str>)?;
    let auto_import = CheckMenuItemBuilder::new("Toggle auto import")
        .id("auto-import")
        .checked(shell_prefs.auto_import)
        .build(app)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Reveal", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &auto_import, &separator, &quit])?;

    app.manage(TrayMenuState {
        auto_import_item: auto_import.clone(),
    });

    let mut builder = TrayIconBuilder::new()
        .tooltip("Reveal")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "auto-import" => {
                show_main_window(app);
                let _ = app.emit("toggle-auto-import-requested", serde_json::json!({}));
            }
            "quit" => app.exit(0),
            _ => {}
        });

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
