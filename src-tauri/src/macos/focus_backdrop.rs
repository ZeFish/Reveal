#![cfg(target_os = "macos")]

use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use objc::{class, msg_send, sel, sel_impl};
use objc::runtime::{Object, YES};
use std::ptr::null_mut;
use std::sync::Mutex;

static PANEL: Mutex<usize> = Mutex::new(0);

const NS_BORDERLESS_WINDOW_MASK: u64 = 0;
const NS_NONACTIVATING_PANEL_MASK: u64 = 1 << 7;
const NS_BACKING_STORE_BUFFERED: u64 = 2;
const NS_NORMAL_WINDOW_LEVEL: i64 = 0;
const NS_WINDOW_BELOW: i64 = -1;

// NSVisualEffectMaterialHUDWindow
const NS_VISUAL_EFFECT_MATERIAL_HUD_WINDOW: i64 = 13;
// NSVisualEffectBlendingModeBehindWindow
const NS_VISUAL_EFFECT_BLENDING_MODE_BEHIND_WINDOW: i64 = 0;
// NSVisualEffectStateActive
const NS_VISUAL_EFFECT_STATE_ACTIVE: i64 = 1;

/// How much colour the Focus Mode backdrop keeps — 0.0 was full grayscale
/// (Francis: too hard), 1.0 is untouched. A soft desaturation still reads as
/// quiet/receded without going flat gray.
const FOCUS_BACKDROP_SATURATION: f64 = 0.35;

const NS_VIEW_WIDTH_SIZABLE: u64 = 1 << 1;
const NS_VIEW_HEIGHT_SIZABLE: u64 = 1 << 4;
const NS_WINDOW_COLLECTION_CAN_JOIN_ALL_SPACES: u64 = 1 << 0;
const NS_WINDOW_COLLECTION_STATIONARY: u64 = 1 << 4;
const NS_WINDOW_COLLECTION_IGNORES_CYCLE: u64 = 1 << 6;
const NS_WINDOW_COLLECTION_FULL_SCREEN_AUXILIARY: u64 = 1 << 8;

pub fn show_below(ns_window: *mut Object) -> Result<(), String> {
    if ns_window.is_null() {
        return Err("main NSWindow unavailable".into());
    }

    unsafe {
        let panel = ensure_panel()?;
        let frame = all_screens_frame();
        let _: () = msg_send![panel, setFrame: frame display: YES];

        let window_number: i64 = msg_send![ns_window, windowNumber];
        let is_visible: bool = msg_send![panel, isVisible];
        if is_visible {
            let _: () = msg_send![panel, orderWindow: NS_WINDOW_BELOW relativeTo: window_number];
        } else {
            let _: () = msg_send![panel, setAlphaValue: 0.0f64];
            let _: () = msg_send![panel, orderWindow: NS_WINDOW_BELOW relativeTo: window_number];
            animate_alpha(panel, 1.0, 0.25);
        }
    }

    Ok(())
}

pub fn hide() {
    unsafe {
        let panel = *PANEL.lock().unwrap() as *mut Object;
        if panel.is_null() {
            return;
        }
        let is_visible: bool = msg_send![panel, isVisible];
        if !is_visible {
            return;
        }
        animate_alpha(panel, 0.0, 0.2);
        let _: () = msg_send![panel, orderOut: null_mut::<Object>()];
    }
}

unsafe fn ensure_panel() -> Result<*mut Object, String> {
    let mut slot = PANEL.lock().unwrap();
    let existing = *slot as *mut Object;
    if !existing.is_null() {
        return Ok(existing);
    }

    let frame = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(100.0, 100.0));
    let style = NS_BORDERLESS_WINDOW_MASK | NS_NONACTIVATING_PANEL_MASK;
    let panel: *mut Object = msg_send![class!(NSPanel), alloc];
    let panel: *mut Object = msg_send![panel,
        initWithContentRect: frame
        styleMask: style
        backing: NS_BACKING_STORE_BUFFERED
        defer: false
    ];

    if panel.is_null() {
        return Err("failed to create focus NSPanel".into());
    }

    let _: () = msg_send![panel, setReleasedWhenClosed: false];
    let _: () = msg_send![panel, setOpaque: false];
    let clear: *mut Object = msg_send![class!(NSColor), clearColor];
    let _: () = msg_send![panel, setBackgroundColor: clear];
    let _: () = msg_send![panel, setHasShadow: false];
    let _: () = msg_send![panel, setIgnoresMouseEvents: true];
    let _: () = msg_send![panel, setLevel: NS_NORMAL_WINDOW_LEVEL];
    let behavior = NS_WINDOW_COLLECTION_CAN_JOIN_ALL_SPACES
        | NS_WINDOW_COLLECTION_STATIONARY
        | NS_WINDOW_COLLECTION_IGNORES_CYCLE
        | NS_WINDOW_COLLECTION_FULL_SCREEN_AUXILIARY;
    let _: () = msg_send![panel, setCollectionBehavior: behavior];

    let content_frame: CGRect = msg_send![panel, contentLayoutRect];
    let container: *mut Object = msg_send![class!(NSView), alloc];
    let container: *mut Object = msg_send![container, initWithFrame: content_frame];
    let _: () = msg_send![container, setAutoresizingMask: NS_VIEW_WIDTH_SIZABLE | NS_VIEW_HEIGHT_SIZABLE];

    let blur: *mut Object = msg_send![class!(NSVisualEffectView), alloc];
    let blur: *mut Object = msg_send![blur, initWithFrame: content_frame];
    let _: () = msg_send![blur, setMaterial: NS_VISUAL_EFFECT_MATERIAL_HUD_WINDOW];
    let _: () = msg_send![blur, setBlendingMode: NS_VISUAL_EFFECT_BLENDING_MODE_BEHIND_WINDOW];
    let _: () = msg_send![blur, setState: NS_VISUAL_EFFECT_STATE_ACTIVE];
    let _: () = msg_send![blur, setAutoresizingMask: NS_VIEW_WIDTH_SIZABLE | NS_VIEW_HEIGHT_SIZABLE];
    let _: () = msg_send![blur, setWantsLayer: true];
    install_desaturation_filter(blur);
    let _: () = msg_send![container, addSubview: blur];

    let dim: *mut Object = msg_send![class!(NSView), alloc];
    let dim: *mut Object = msg_send![dim, initWithFrame: content_frame];
    let _: () =
        msg_send![dim, setAutoresizingMask: NS_VIEW_WIDTH_SIZABLE | NS_VIEW_HEIGHT_SIZABLE];
    let _: () = msg_send![dim, setWantsLayer: true];
    let layer: *mut Object = msg_send![dim, layer];
    let color: *mut Object =
        msg_send![class!(NSColor), colorWithCalibratedWhite: 0.0f64 alpha: 0.10f64];
    let cg_color: *mut Object = msg_send![color, CGColor];
    let _: () = msg_send![layer, setBackgroundColor: cg_color];
    let _: () = msg_send![container, addSubview: dim];

    let _: () = msg_send![panel, setContentView: container];
    *slot = panel as usize;
    Ok(panel)
}

unsafe fn animate_alpha(panel: *mut Object, alpha: f64, duration: f64) {
    let context: *mut Object = msg_send![class!(NSAnimationContext), currentContext];
    let _: () = msg_send![context, setDuration: duration];
    let animator: *mut Object = msg_send![panel, animator];
    let _: () = msg_send![animator, setAlphaValue: alpha];
}

unsafe fn ns_string(value: &str) -> *mut Object {
    let bytes = std::ffi::CString::new(value).unwrap();
    msg_send![class!(NSString), stringWithUTF8String: bytes.as_ptr()]
}

unsafe fn install_desaturation_filter(view: *mut Object) {
    let layer: *mut Object = msg_send![view, layer];
    if layer.is_null() {
        return;
    }

    let filter_name = ns_string("CIColorControls");
    let filter: *mut Object = msg_send![class!(CIFilter), filterWithName: filter_name];
    if filter.is_null() {
        return;
    }

    let saturation_key = ns_string("inputSaturation");
    let saturation: *mut Object =
        msg_send![class!(NSNumber), numberWithDouble: FOCUS_BACKDROP_SATURATION];
    let _: () = msg_send![filter, setValue: saturation forKey: saturation_key];

    let filters: *mut Object = msg_send![class!(NSArray), arrayWithObject: filter];
    let _: () = msg_send![layer, setBackgroundFilters: filters];
}

unsafe fn all_screens_frame() -> CGRect {
    let screens: *mut Object = msg_send![class!(NSScreen), screens];
    let count: usize = msg_send![screens, count];

    let mut min_x = 0.0;
    let mut min_y = 0.0;
    let mut max_x = 0.0;
    let mut max_y = 0.0;

    for i in 0..count {
        let screen: *mut Object = msg_send![screens, objectAtIndex: i];
        let frame: CGRect = msg_send![screen, frame];
        if i == 0 {
            min_x = frame.origin.x;
            min_y = frame.origin.y;
            max_x = frame.origin.x + frame.size.width;
            max_y = frame.origin.y + frame.size.height;
        } else {
            min_x = min_x.min(frame.origin.x);
            min_y = min_y.min(frame.origin.y);
            max_x = max_x.max(frame.origin.x + frame.size.width);
            max_y = max_y.max(frame.origin.y + frame.size.height);
        }
    }

    CGRect::new(
        &CGPoint::new(min_x, min_y),
        &CGSize::new(max_x - min_x, max_y - min_y),
    )
}
