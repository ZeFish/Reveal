#![cfg(target_os = "macos")]

use objc::runtime::{Object, YES};
use objc::{msg_send, sel, sel_impl};

const NS_WINDOW_CLOSE_BUTTON: i64 = 0;
const NS_WINDOW_MINIATURIZE_BUTTON: i64 = 1;
const NS_WINDOW_ZOOM_BUTTON: i64 = 2;

pub fn style(ns_window: *mut Object) -> Result<(), String> {
    if ns_window.is_null() {
        return Err("main NSWindow unavailable".into());
    }

    unsafe {
        let close: *mut Object = msg_send![ns_window, standardWindowButton: NS_WINDOW_CLOSE_BUTTON];
        let mini: *mut Object = msg_send![ns_window, standardWindowButton: NS_WINDOW_MINIATURIZE_BUTTON];
        let zoom: *mut Object = msg_send![ns_window, standardWindowButton: NS_WINDOW_ZOOM_BUTTON];
        if close.is_null() || mini.is_null() || zoom.is_null() {
            return Ok(());
        }
        // AppKit colors the complete titlebar button group when any titlebar
        // area is hovered. Reveal draws its own exact-hover controls instead,
        // while these hidden buttons retain the native window behavior.
        for button in [close, mini, zoom] {
            let _: () = msg_send![button, setHidden: YES];
        }
    }

    Ok(())
}
