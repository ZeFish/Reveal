//! The libraw backend — the SAME decode the Python reference uses (rawpy is
//! a libraw wrapper), replicated parameter-for-parameter:
//!
//!   output_color=ACES · 16-bit · no_auto_bright · gamma (1,1) · camera WB
//!
//! M1 verification showed rawler's camera matrix reads green-teal on Fuji
//! RAFs while this path matches the Python render — so libraw is the
//! primary decoder (plan risk R4, plan B fired). All unsafe stays inside
//! this module.

use std::ffi::CString;
use std::path::Path;

use crate::{DecodeError, LinearImage, Primaries, RawDecoder};

#[allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code)]
mod ffi {
    include!(concat!(env!("OUT_DIR"), "/libraw_bindings.rs"));
}

pub struct LibrawDecoder;

/// rawpy `ColorSpace.ACES` — libraw output_color index.
const OUTPUT_COLOR_ACES: i32 = 6;

impl RawDecoder for LibrawDecoder {
    fn decode_linear(&self, path: &Path, fast: bool) -> Result<LinearImage, DecodeError> {
        let cpath = CString::new(path.to_string_lossy().as_bytes())
            .map_err(|_| DecodeError::Decode("path contains NUL".into()))?;

        unsafe {
            let lr = ffi::libraw_init(0);
            if lr.is_null() {
                return Err(DecodeError::Decode("libraw_init failed".into()));
            }
            // Ensure cleanup on every exit path below.
            let guard = LibrawGuard(lr);

            check(ffi::libraw_open_file(lr, cpath.as_ptr()), "open_file")?;

            let p = &mut (*lr).params;
            p.output_color = OUTPUT_COLOR_ACES;
            p.output_bps = 16;
            p.no_auto_bright = 1;
            p.use_camera_wb = 1;
            // gamma (1,1) = linear, rawpy convention (gamm[0] = 1/power).
            p.gamm[0] = 1.0;
            p.gamm[1] = 1.0;
            // Preview path: half-resolution decode. This skips the full
            // demosaic (Fuji X-Trans Markesteijn 3-pass is ~20 s at full res);
            // the half-res image is still ~3000 px — well above the 2048 px
            // preview/export — so downscaling hides any quality difference.
            if fast {
                p.half_size = 1;
            }

            check(ffi::libraw_unpack(lr), "unpack")?;
            check(ffi::libraw_dcraw_process(lr), "dcraw_process")?;

            let mut err: i32 = 0;
            let img = ffi::libraw_dcraw_make_mem_image(lr, &mut err);
            if img.is_null() {
                return Err(DecodeError::Develop(format!(
                    "dcraw_make_mem_image: {}",
                    strerror(err)
                )));
            }
            let mem = MemImageGuard(img);

            let h = (*img).height as usize;
            let w = (*img).width as usize;
            let colors = (*img).colors as usize;
            let bits = (*img).bits;
            if colors != 3 || bits != 16 {
                return Err(DecodeError::Develop(format!(
                    "unexpected mem image: {colors} colors, {bits} bits"
                )));
            }

            let px = std::slice::from_raw_parts((*img).data.as_ptr() as *const u16, w * h * 3);
            const INV: f32 = 1.0 / 65535.0;
            let data: Vec<f32> = px.iter().map(|&v| v as f32 * INV).collect();

            drop(mem);
            drop(guard);

            Ok(LinearImage {
                width: w as u32,
                height: h as u32,
                data,
                primaries: Primaries::Aces2065_1,
            })
        }
    }
}

pub struct ThumbPreview {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

/// Extract the camera's embedded preview — JPEG when available, otherwise
/// convert libraw's bitmap thumb to BMP bytes (still fast and browser-safe).
pub fn extract_thumb_preview(path: &Path) -> Result<ThumbPreview, DecodeError> {
    let cpath = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|_| DecodeError::Decode("path contains NUL".into()))?;
    unsafe {
        let lr = ffi::libraw_init(0);
        if lr.is_null() {
            return Err(DecodeError::Decode("libraw_init failed".into()));
        }
        let _guard = LibrawGuard(lr);
        check(ffi::libraw_open_file(lr, cpath.as_ptr()), "open_file")?;
        check(ffi::libraw_unpack_thumb(lr), "unpack_thumb")?;
        let mut err: i32 = 0;
        let img = ffi::libraw_dcraw_make_mem_thumb(lr, &mut err);
        if img.is_null() {
            return Err(DecodeError::Develop(format!(
                "make_mem_thumb: {}",
                strerror(err)
            )));
        }
        let _mem = MemImageGuard(img);
        match (*img).type_ {
            ffi::LibRaw_image_formats_LIBRAW_IMAGE_JPEG => {
                let bytes = std::slice::from_raw_parts((*img).data.as_ptr(), (*img).data_size as usize).to_vec();
                Ok(ThumbPreview { bytes, mime: "image/jpeg" })
            }
            ffi::LibRaw_image_formats_LIBRAW_IMAGE_BITMAP => {
                let bytes = bmp_from_libraw_bitmap(img)?;
                Ok(ThumbPreview { bytes, mime: "image/bmp" })
            }
            other => Err(DecodeError::Develop(format!(
                "unsupported embedded thumb format: {other}"
            ))),
        }
    }
}

/// Legacy helper kept for tests/examples that explicitly expect JPEG thumbs.
pub fn extract_thumb_jpeg(path: &Path) -> Result<Vec<u8>, DecodeError> {
    let p = extract_thumb_preview(path)?;
    if p.mime == "image/jpeg" {
        Ok(p.bytes)
    } else {
        Err(DecodeError::Develop("embedded thumb is not JPEG".into()))
    }
}

/// The capture timestamp (unix seconds) from the RAW's metadata — a cheap
/// header read (open_file only, no unpack). None when the camera left it
/// empty.
pub fn capture_timestamp(path: &Path) -> Option<i64> {
    let cpath = CString::new(path.to_string_lossy().as_bytes()).ok()?;
    unsafe {
        let lr = ffi::libraw_init(0);
        if lr.is_null() {
            return None;
        }
        let _guard = LibrawGuard(lr);
        if ffi::libraw_open_file(lr, cpath.as_ptr()) != 0 {
            return None;
        }
        let ts = (*lr).other.timestamp as i64;
        (ts > 0).then_some(ts)
    }
}

/// Visible RAW dimensions from libraw's header metadata; no pixel decode.
pub fn capture_dimensions(path: &Path) -> Option<(u32, u32)> {
    let cpath = CString::new(path.to_string_lossy().as_bytes()).ok()?;
    unsafe {
        let lr = ffi::libraw_init(0);
        if lr.is_null() {
            return None;
        }
        let _guard = LibrawGuard(lr);
        if ffi::libraw_open_file(lr, cpath.as_ptr()) != 0 {
            return None;
        }
        let sizes = &(*lr).sizes;
        let width = u32::from(sizes.width);
        let height = u32::from(sizes.height);
        (width > 0 && height > 0).then_some((width, height))
    }
}

struct LibrawGuard(*mut ffi::libraw_data_t);
impl Drop for LibrawGuard {
    fn drop(&mut self) {
        unsafe { ffi::libraw_close(self.0) }
    }
}

struct MemImageGuard(*mut ffi::libraw_processed_image_t);
impl Drop for MemImageGuard {
    fn drop(&mut self) {
        unsafe { ffi::libraw_dcraw_clear_mem(self.0) }
    }
}

fn strerror(code: i32) -> String {
    unsafe {
        let s = ffi::libraw_strerror(code);
        if s.is_null() {
            format!("libraw error {code}")
        } else {
            std::ffi::CStr::from_ptr(s).to_string_lossy().into_owned()
        }
    }
}

fn check(code: i32, stage: &str) -> Result<(), DecodeError> {
    if code == 0 {
        Ok(())
    } else {
        Err(DecodeError::Decode(format!("{stage}: {}", strerror(code))))
    }
}

unsafe fn bmp_from_libraw_bitmap(img: *mut ffi::libraw_processed_image_t) -> Result<Vec<u8>, DecodeError> {
    let w = (*img).width as usize;
    let h = (*img).height as usize;
    let colors = (*img).colors as usize;
    let bits = (*img).bits as usize;
    if w == 0 || h == 0 {
        return Err(DecodeError::Develop("invalid bitmap thumb dimensions".into()));
    }
    if bits != 8 && bits != 16 {
        return Err(DecodeError::Develop(format!("unsupported bitmap thumb bits: {bits}")));
    }
    if colors == 0 {
        return Err(DecodeError::Develop("invalid bitmap thumb channels".into()));
    }

    let bytes_per_chan = bits / 8;
    let src_stride = w
        .checked_mul(colors)
        .and_then(|v| v.checked_mul(bytes_per_chan))
        .ok_or_else(|| DecodeError::Develop("bitmap thumb overflow".into()))?;
    let src_len = src_stride
        .checked_mul(h)
        .ok_or_else(|| DecodeError::Develop("bitmap thumb overflow".into()))?;
    let data_size = (*img).data_size as usize;
    if data_size < src_len {
        return Err(DecodeError::Develop(format!(
            "bitmap thumb truncated: expected {src_len} bytes, got {data_size}"
        )));
    }
    let src = std::slice::from_raw_parts((*img).data.as_ptr(), src_len);

    // 24-bit BMP row stride (4-byte aligned).
    let row_raw = w
        .checked_mul(3)
        .ok_or_else(|| DecodeError::Develop("bitmap thumb row overflow".into()))?;
    let row_stride = (row_raw + 3) & !3;
    let pixel_bytes = row_stride
        .checked_mul(h)
        .ok_or_else(|| DecodeError::Develop("bitmap thumb size overflow".into()))?;
    let file_size = 14usize
        .checked_add(40)
        .and_then(|v| v.checked_add(pixel_bytes))
        .ok_or_else(|| DecodeError::Develop("bitmap thumb file overflow".into()))?;

    let mut out = Vec::with_capacity(file_size);
    // BITMAPFILEHEADER (14)
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(file_size as u32).to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&54u32.to_le_bytes()); // pixel data offset
    // BITMAPINFOHEADER (40)
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(w as i32).to_le_bytes());
    out.extend_from_slice(&(h as i32).to_le_bytes()); // bottom-up
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&24u16.to_le_bytes()); // bpp
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
    out.extend_from_slice(&2835u32.to_le_bytes()); // 72 DPI
    out.extend_from_slice(&2835u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());

    let pad = [0u8; 3];
    for y in (0..h).rev() {
        let row_off = y * src_stride;
        for x in 0..w {
            let px = row_off + x * colors * bytes_per_chan;
            let read_chan = |chan: usize| -> u8 {
                let c = chan.min(colors.saturating_sub(1));
                let off = px + c * bytes_per_chan;
                if bits == 8 {
                    src[off]
                } else {
                    src[off + 1] // high byte of little-endian u16
                }
            };
            let r = read_chan(0);
            let g = read_chan(1);
            let b = read_chan(2);
            out.push(b);
            out.push(g);
            out.push(r);
        }
        let padding = row_stride - row_raw;
        out.extend_from_slice(&pad[..padding]);
    }

    Ok(out)
}
