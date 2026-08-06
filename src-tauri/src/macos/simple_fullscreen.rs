#![cfg(target_os = "macos")]

// Lightroom-style "simple" fullscreen: hide the menu bar and Dock and let the
// window grow to cover the whole display IN PLACE — no Spaces animation, no new
// Space. That is the difference from `NSWindow toggleFullScreen:` (what Tauri's
// `set_fullscreen(true)` calls), which slides the app into its own Space and
// feels like a maximize.
//
// The window frame itself is grown on the Tauri side with the cross-platform
// window API; the only thing that needs AppKit here is the app-wide
// presentation options, which are a plain bitmask — so there is no NSRect
// marshalling to get wrong.

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use std::sync::Mutex;

// NSApplicationPresentationOptions bits.
const NS_APP_PRESENTATION_DEFAULT: u64 = 0;
const NS_APP_PRESENTATION_HIDE_DOCK: u64 = 1 << 1;
const NS_APP_PRESENTATION_HIDE_MENU_BAR: u64 = 1 << 3;

// NSWindowStyleMask.borderless — dropping the titled/resizable/etc. bits
// removes the titlebar hairline, rounded corners and drop shadow AppKit
// otherwise draws around a normal window, even one resized to cover the
// whole display.
const NS_WINDOW_STYLE_MASK_BORDERLESS: u64 = 0;

static SAVED_STYLE_MASK: Mutex<Option<u64>> = Mutex::new(None);

/// Hide the Dock and menu bar outright — not the auto-hide variant, which
/// still leaves a hover-to-reveal sliver (status icons peeking through) at
/// the top edge. Francis wants zero OS chrome while fullscreen, Lightroom-
/// style: nothing to see or hover-reveal until Escape exits.
pub fn enter() -> Result<(), String> {
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        if app.is_null() {
            return Err("NSApplication unavailable".into());
        }
        let opts = NS_APP_PRESENTATION_HIDE_DOCK | NS_APP_PRESENTATION_HIDE_MENU_BAR;
        let _: () = msg_send![app, setPresentationOptions: opts];
    }
    Ok(())
}

/// Restore the standard Dock + menu bar.
pub fn exit() -> Result<(), String> {
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        if app.is_null() {
            return Err("NSApplication unavailable".into());
        }
        let _: () = msg_send![app, setPresentationOptions: NS_APP_PRESENTATION_DEFAULT];
    }
    Ok(())
}

/// Strip the window down to a bare borderless surface — no titlebar, no
/// rounded corners, no drop shadow — so the display-filling frame reads as
/// true edge-to-edge coverage instead of a maximized normal window.
pub fn strip_chrome(ns_window: *mut Object) {
    unsafe {
        let mask: u64 = msg_send![ns_window, styleMask];
        *SAVED_STYLE_MASK.lock().unwrap() = Some(mask);
        let _: () = msg_send![ns_window, setStyleMask: NS_WINDOW_STYLE_MASK_BORDERLESS];
        let _: () = msg_send![ns_window, setHasShadow: false];
    }
}

/// Restore the window's normal chrome after leaving simple fullscreen.
pub fn restore_chrome(ns_window: *mut Object) {
    unsafe {
        if let Some(mask) = SAVED_STYLE_MASK.lock().unwrap().take() {
            let _: () = msg_send![ns_window, setStyleMask: mask];
        }
        let _: () = msg_send![ns_window, setHasShadow: true];
    }
}
