#![cfg(target_os = "macos")]

use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};

/// Writes JPEG bytes straight to the general NSPasteboard as an image —
/// bypasses the WebView's Clipboard API entirely (see the doc comment on
/// `copy_developed_preview_to_clipboard` in lib.rs for why).
pub fn write_jpeg_image(jpeg_bytes: &[u8]) -> Result<(), String> {
    unsafe {
        let data: *mut Object = msg_send![class!(NSData), dataWithBytes: jpeg_bytes.as_ptr() length: jpeg_bytes.len()];
        if data.is_null() {
            return Err("failed to wrap preview bytes in NSData".into());
        }

        let image: *mut Object = msg_send![class!(NSImage), alloc];
        let image: *mut Object = msg_send![image, initWithData: data];
        if image.is_null() {
            return Err("NSImage could not decode the developed preview".into());
        }

        let pasteboard: *mut Object = msg_send![class!(NSPasteboard), generalPasteboard];
        let _: () = msg_send![pasteboard, clearContents];

        let array: *mut Object = msg_send![class!(NSArray), arrayWithObject: image];
        let wrote: bool = msg_send![pasteboard, writeObjects: array];
        if !wrote {
            return Err("NSPasteboard declined to write the image".into());
        }
    }
    Ok(())
}
