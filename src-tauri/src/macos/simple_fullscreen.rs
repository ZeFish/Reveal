#![cfg(target_os = "macos")]

// Lightroom-style borderless fullscreen: hide the menu bar and Dock and let the
// window cover the whole display in place — no Spaces animation, no new Space.
// Setting the native screen frame directly avoids coordinate inversion and
// DPI calculation issues, ensuring 100% display coverage including the system bar.
// Keeping the normal window level and explicitly asserting key-window status
// guarantees keyboard events (like pressing 'f' to exit) continue working.

use core_graphics::geometry::CGRect;
use objc::runtime::{Object, YES};
use objc::{class, msg_send, sel, sel_impl};
use std::ptr::null_mut;
use std::sync::Mutex;

// NSApplicationPresentationOptions bits.
const NS_APP_PRESENTATION_DEFAULT: u64 = 0;
const NS_APP_PRESENTATION_HIDE_DOCK: u64 = 1 << 1;
const NS_APP_PRESENTATION_HIDE_MENU_BAR: u64 = 1 << 3;

static SAVED_FRAME: Mutex<Option<CGRect>> = Mutex::new(None);
static SAVED_SHADOW: Mutex<Option<bool>> = Mutex::new(None);

/// Enter Lightroom-style simple fullscreen: hide Dock & menu bar,
/// expand frame to full screen, and ensure key window focus is preserved.
pub fn enter(ns_window: *mut Object) -> Result<(), String> {
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        if app.is_null() {
            return Err("NSApplication unavailable".into());
        }
        let opts = NS_APP_PRESENTATION_HIDE_DOCK | NS_APP_PRESENTATION_HIDE_MENU_BAR;
        let _: () = msg_send![app, setPresentationOptions: opts];

        if !ns_window.is_null() {
            // Save state if not already saved (prevents overwriting on re-entrant calls)
            if SAVED_FRAME.lock().unwrap().is_none() {
                let frame: CGRect = msg_send![ns_window, frame];
                *SAVED_FRAME.lock().unwrap() = Some(frame);
            }
            if SAVED_SHADOW.lock().unwrap().is_none() {
                let has_shadow: bool = msg_send![ns_window, hasShadow];
                *SAVED_SHADOW.lock().unwrap() = Some(has_shadow);
            }

            let _: () = msg_send![ns_window, setHasShadow: false];

            // Fill the current monitor's full screen frame in place (Cocoa coords)
            let screen: *mut Object = msg_send![ns_window, screen];
            if !screen.is_null() {
                let screen_frame: CGRect = msg_send![screen, frame];
                let _: () = msg_send![ns_window, setFrame: screen_frame display: YES animate: false];
            }

            // Explicitly assert key window status and activate app so keyboard
            // shortcuts (e.g. pressing 'f', Escape, etc.) continue to work seamlessly.
            let _: () = msg_send![ns_window, makeKeyAndOrderFront: null_mut::<Object>()];
            let _: () = msg_send![ns_window, makeKeyWindow];
            let _: () = msg_send![app, activateIgnoringOtherApps: YES];
        }
    }
    Ok(())
}

/// Restore the window's normal frame, shadow, standard Dock + menu bar, and key focus.
pub fn exit(ns_window: *mut Object) -> Result<(), String> {
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        if app.is_null() {
            return Err("NSApplication unavailable".into());
        }
        let _: () = msg_send![app, setPresentationOptions: NS_APP_PRESENTATION_DEFAULT];

        if !ns_window.is_null() {
            // Restore window frame
            if let Some(frame) = SAVED_FRAME.lock().unwrap().take() {
                let _: () = msg_send![ns_window, setFrame: frame display: YES animate: false];
            }

            // Restore shadow
            if let Some(has_shadow) = SAVED_SHADOW.lock().unwrap().take() {
                let _: () = msg_send![ns_window, setHasShadow: has_shadow];
            } else {
                let _: () = msg_send![ns_window, setHasShadow: true];
            }

            // Ensure window retains key focus upon exiting fullscreen
            let _: () = msg_send![ns_window, makeKeyAndOrderFront: null_mut::<Object>()];
            let _: () = msg_send![ns_window, makeKeyWindow];
            let _: () = msg_send![app, activateIgnoringOtherApps: YES];
        }
    }
    Ok(())
}
