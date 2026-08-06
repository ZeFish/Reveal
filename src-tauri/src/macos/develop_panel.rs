#![cfg(target_os = "macos")]

use objc::runtime::{Object, YES};
use objc::{msg_send, sel, sel_impl};

const NS_WINDOW_CLOSE_BUTTON: i64 = 0;
const NS_WINDOW_MINIATURIZE_BUTTON: i64 = 1;
const NS_WINDOW_ZOOM_BUTTON: i64 = 2;

/// Make the Develop panel behave like a macOS utility panel.
pub fn configure(ns_window: *mut Object) -> Result<(), String> {
    if ns_window.is_null() {
        return Err("Develop NSWindow unavailable".into());
    }

    unsafe {
        for button_type in [
            NS_WINDOW_CLOSE_BUTTON,
            NS_WINDOW_MINIATURIZE_BUTTON,
            NS_WINDOW_ZOOM_BUTTON,
        ] {
            let button: *mut Object = msg_send![ns_window, standardWindowButton: button_type];
            if !button.is_null() {
                let _: () = msg_send![button, setHidden: YES];
            }
        }

        // AppKit hides this window only when Reveal itself deactivates, not
        // when focus moves between the main window and the Develop panel.
        let _: () = msg_send![ns_window, setHidesOnDeactivate: YES];
    }

    Ok(())
}
