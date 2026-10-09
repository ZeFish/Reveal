//! The Tauri shell: app setup, the window/tray/menu/focus machinery, the
//! `reveal://` protocol handler, and the one `generate_handler!` that lists
//! every command.
//!
//! Submodules:
//! - `shell_prefs`: preferences file management and auto-import settings
//! - `garden`: Garden cloud authentication and account connection
//! - `window`: focus mode, fullscreen, traffic lights, window visibility
//! - `menu`: application menu setup
//! - `tray`: system menu bar tray setup
//! - `system`: system utilities, open in finder/editor, notifications
//! - `engine_cmds`: engine orchestration, presets, sidecars, frame EXIF

#![allow(unexpected_cfgs)]

use tauri::http::Response as HttpResponse;
pub(crate) use tauri::ipc::Response as IpcResponse;
use tauri::{Emitter, Manager};

mod apple_photos;
mod catalog; // the library on disk: folders, ratings, roots, the index
mod cull; // AI-assisted culling
mod engine_cmds;
mod export; // exporting developed photos
pub mod google_photos; // Google Photos export integration
pub mod immich; // Immich integration
mod import_cards; // ingesting memory cards
mod metadata; // captions and tags
mod preview; // every on-disk derivative of a photo
mod publishing; // stories, the vault, Garden
mod daily_note;
mod photo_cache;
mod preset;
mod story;
mod fd_limit;
mod photo; // a photo: its identity, where it lives, what can be done to it
mod panel_sync; // the detached develop panel follows the main window away and back
mod photo_writes; // what is owed to a photo's files, and when (debounced, flushed on exit)
mod thumb; // the `reveal://thumb` handler: request, cache, ladder of sources
mod thumb_queue; // who decodes a grid thumbnail next
mod xmp_preset;
mod log_capture;

mod garden;
mod menu;
mod shell_prefs;
mod system;
mod tray;
mod window;

#[cfg(target_os = "macos")]
mod macos;

pub(crate) use engine_cmds::*;
pub(crate) use garden::*;
pub(crate) use preview::*;
pub(crate) use shell_prefs::*;
pub(crate) use system::*;
pub(crate) use window::*;

/// Run blocking work (file I/O, usually on the NAS) off the async runtime.
///
/// An `async fn` command that touches a network volume directly parks one of
/// the runtime's few worker threads for as long as the NAS takes to answer. When
/// the NAS stops answering, an NFS call can take minutes to time out, a few
/// edits take every worker, and then no command at all gets a turn: renders
/// never return and the spinner never stops. The blocking pool is wide, and
/// holding one of its threads costs nothing else.
pub(crate) async fn blocking<T, F>(work: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| e.to_string())?
}

/// What the interface reads from an `app-error` event: `{ "message": … }`.
pub(crate) fn app_error_payload(message: impl Into<String>) -> serde_json::Value {
    serde_json::json!({ "message": message.into() })
}

/// Tell the interface something went wrong that nobody asked about: it shows the message as a
/// notice. For failures with no caller to return the error to (a background write, a menu action,
/// a deep link).
pub(crate) fn app_error<R: tauri::Runtime>(emitter: &impl Emitter<R>, message: impl Into<String>) {
    let _ = emitter.emit("app-error", app_error_payload(message));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Intercept stderr & stdout so logs are captured in memory for the activity log window
    log_capture::init_log_capture();

    // Before anything opens a file: a Finder-launched app starts with 256 (see fd_limit.rs).
    if let Some(limit) = fd_limit::raise_open_file_limit() {
        eprintln!("open files allowed: {limit}");
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        // Updates: the frontend drives check → download → relaunch, so the
        // experience is Reveal's own (see src/lib/updater.svelte.js).
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(|app, shortcut, event| {
            use tauri_plugin_global_shortcut::{Code, Modifiers, ShortcutState};
            if event.state() == ShortcutState::Pressed {
                if shortcut.mods.contains(Modifiers::ALT) && shortcut.key == Code::KeyR {
                    toggle_window_visibility(app);
                }
            }
        }).build())
        .plugin(tauri_plugin_window_state::Builder::default().with_denylist(&["import-panel", "settings-panel"]).build())
        .setup(|app| {
            log_capture::register_app_handle(app.handle().clone());
            apple_photos::init(app.handle())?;
            menu::setup_main_menu(app).map_err(|e| e.to_string())?;
            tray::setup_tray(app).map_err(|e| e.to_string())?;
            panel_sync::start(app.handle().clone());

            // The browser sign-in flow (standard.garden/connect/reveal) hands a
            // freshly-minted key back via `reveal://garden-callback?key=...` —
            // the Tauri counterpart of Swift's `application(_:open:)` (which
            // this app now replaces — same scheme, one app owns it).
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
                        let given = url
                            .query_pairs()
                            .find(|(k, _)| k == "state")
                            .map(|(_, v)| v.into_owned());
                        if !take_connect_nonce(&dl_app, given.as_deref()) {
                            app_error(
                                &dl_app,
                                "Garden connection ignored: it was not started from Reveal, or it expired. Use “Connect via browser” and try again.",
                            );
                            continue;
                        }
                        let app_for_task = dl_app.clone();
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = sign_in_with_key(&app_for_task, &key).await {
                                app_error(&app_for_task, format!("Garden sign-in: {e}"));
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
            app.manage(GardenConnectState::default());
            app.manage(OpenFileState::default());
            app.manage(ImportState::default());
            app.manage(ImportCancelState::default());
            app.manage(ExportState::default());
            app.manage(CullState::default());
            app.manage(CullCancelState::default());
            let photo_writes = photo_writes::PhotoWrites::start(
                photo_writes::AppDisk { app: app.handle().clone() },
                photo_writes::QUIET,
            );
            photo_writes::install(photo_writes.clone());
            app.manage(photo_writes::PhotoWritesState(photo_writes));
            app.manage(thumb_queue::ThumbQueueState(std::sync::Arc::new(thumb_queue::ThumbQueue::new(6))));
            #[cfg(target_os = "macos")]
            macos::volume_watcher::start(app.handle().clone());

            let data_dir = resolve_data_dir(app);
            let luts_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("app_data_dir: {e}"))?
                .join("luts");
            let engine = reveal_engine::Engine::new(&data_dir, &luts_dir)
                .map_err(|e| format!("engine init ({}): {e:#}", data_dir.display()))?;
            eprintln!("engine ready — backend: {}", engine.backend_name());
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
            schedule_cache_prune(app.handle());
            Ok(())
        })
        .register_asynchronous_uri_scheme_protocol("reveal", |_ctx, request, responder| {
            let host = request.uri().host().unwrap_or_default().to_string();
            match host.as_str() {
                "ping" => {
                    let response = HttpResponse::builder()
                        .header("Content-Type", "image/svg+xml")
                        .body(PING_SVG.as_bytes().to_vec())
                        .unwrap();
                    responder.respond(response);
                }
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
                        .header("Access-Control-Allow-Origin", "*")
                        .body(b"ok".to_vec())
                        .unwrap();
                    responder.respond(response);
                }
                "thumb" => thumb::serve(_ctx.app_handle().clone(), &request, responder),
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
            set_focus,
            set_focus_window_presence,
            set_simple_fullscreen,
            hide_contact_sheet,
            hide_window_traffic_lights,
            ping,
            catalog::pick_folder,
            catalog::list_dir,
            catalog::set_rating,
            catalog::adjust_capture_date,
            catalog::tidy_plan,
            catalog::tidy_apply,
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
            export::export_photos,
            export::cancel_exports,
            export::test_immich_connection,
            export::upload_photo_to_immich,
            immich::immich_status,
            immich::immich_albums,
            immich::immich_list,
            google_photos::google_photos_start_auth,
            google_photos::google_photos_test_connection,
            google_photos::google_photos_disconnect,
            google_photos::upload_photo_to_google_photos,
            cull::ai_cull,
            cull::ai_cull_selection,
            cull::cancel_cull,
            reveal_main_window,
            publishing::story_publish_status,
            publishing::story_stems,
            publishing::story_toggle,
            publishing::publish_story,
            publishing::publish_photo,
            garden_sign_in,
            garden_sign_out,
            garden_begin_connect,
            garden_refresh,
            publishing::load_story_note,
            publishing::save_story_note,
            publishing::story_load_theme,
            publishing::story_set_theme,
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
            set_thumb_priority,
            set_visible_thumbs,
            list_profiles,
            list_luts,
            list_presets,
            save_preset,
            delete_preset,
            import_xmp_presets,
            load_sidecar,
            save_recipe,
            clear_recipe,
            queue_clear_development,
            queue_save_recipe,
            cancel_photo_writes,
            preview_versions,
            autoload_path,
            take_open_file,
            list_external_editors,
            open_in_editor,
            load_catalog_note,
            save_catalog_note,
            log_capture::get_log_history,
            log_capture::clear_log_history,
            log_capture::show_log_window,
            log_capture::toggle_devtools
        ])
        .on_window_event(|window, event| {
            #[cfg(target_os = "macos")]
            if matches!(window.label(), "develop-panel") {
                if let Ok(ns_window) = window.ns_window() {
                    let _ = macos::develop_panel::configure(
                        ns_window as *mut objc::runtime::Object,
                    );
                }
            } else if matches!(window.label(), "main" | "settings-panel")
                && matches!(
                    event,
                    tauri::WindowEvent::Resized(_)
                        | tauri::WindowEvent::ScaleFactorChanged { .. }
                        | tauri::WindowEvent::ThemeChanged(_)
                )
            {
                if let Ok(ns_window) = window.ns_window() {
                    let _ =
                        macos::traffic_lights::style(ns_window as *mut objc::runtime::Object);
                }
            }
            #[cfg(target_os = "macos")]
            if let tauri::WindowEvent::Focused(focused) = event {
                let app = window.app_handle().clone();
                let focus_enabled = app
                    .try_state::<FocusState>()
                    .map(|state| *state.0.lock().unwrap())
                    .unwrap_or(false);
                if focus_enabled {
                    if *focused {
                        let has_presence = app
                            .try_state::<FocusPresenceState>()
                            .map(|state| !state.0.lock().unwrap().is_empty())
                            .unwrap_or(false);
                        if has_presence {
                            let _ = apply_focus_backdrop(&app, true);
                        }
                    } else {
                        let delayed_app = app.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            let is_active = unsafe {
                                use objc::{class, msg_send, sel, sel_impl};
                                let ns_app: *mut objc::runtime::Object =
                                    msg_send![class!(NSApplication), sharedApplication];
                                if !ns_app.is_null() {
                                    let is_act: objc::runtime::BOOL = msg_send![ns_app, isActive];
                                    let key_win: *mut objc::runtime::Object =
                                        msg_send![ns_app, keyWindow];
                                    is_act == objc::runtime::YES && !key_win.is_null()
                                } else {
                                    false
                                }
                            };
                            let has_presence = delayed_app
                                .try_state::<FocusPresenceState>()
                                .map(|state| !state.0.lock().unwrap().is_empty())
                                .unwrap_or(false);
                            if !is_active || !has_presence {
                                let main_app = delayed_app.clone();
                                let _ = delayed_app.run_on_main_thread(move || {
                                    let _ = apply_focus_backdrop(&main_app, false);
                                });
                            }
                        });
                    }
                }
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                #[cfg(target_os = "macos")]
                if matches!(window.label(), "main") {
                    macos::focus_backdrop::hide();
                }
                let _ = window.hide();
                if matches!(window.label(), "main") {
                    let importing = window
                        .app_handle()
                        .try_state::<ImportState>()
                        .map(|state| !state.0.lock().unwrap().is_empty())
                        .unwrap_or(false);
                    if importing {
                        show_import_panel(window.app_handle());
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building Reveal")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<photo_writes::PhotoWritesState>().0.flush_all();
            }
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

    #[test]
    fn a_full_frame_costs_its_pixels_plus_a_header() {
        let (w, h) = (2048u32, 1365u32);
        let rgba = vec![128u8; (w * h * 4) as usize];
        let packed = pack_developed_frame(w, h, 24, 0, &rgba);
        assert_eq!(packed.len(), 16 + (w * h * 4) as usize);
        assert!(packed.len() < 12_000_000, "{} bytes", packed.len());
    }
}

#[cfg(test)]
mod app_error_tests {
    use super::app_error_payload;

    #[test]
    fn the_interface_reads_a_message_field() {
        let payload = app_error_payload(format!("Garden sign-in: {}", "expired"));
        assert_eq!(payload, serde_json::json!({ "message": "Garden sign-in: expired" }));
    }
}
