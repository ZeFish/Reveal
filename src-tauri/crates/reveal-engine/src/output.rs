use anyhow::{Context, Result};
use spektrafilm_math::image::ImageBuf;

pub struct RenderOutput {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Pipeline time only, milliseconds.
    pub render_ms: u128,
    /// RAW decode time (0 on decode-cache hit), milliseconds.
    pub decode_ms: u128,
}

pub struct RenderRgbaOutput {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub render_ms: u128,
    pub decode_ms: u128,
}

/// Quantize a pipeline result (display-encoded sRGB, [0,1]) to 8-bit —
/// `round_ties_even` stays numpy-identical with the reference tools.
pub fn quantize_rgb8(img: &ImageBuf) -> Vec<u8> {
    img.data
        .iter()
        .map(|&v| ((f64::from(v).clamp(0.0, 1.0) * 255.0).round_ties_even()) as u8)
        .collect()
}

/// Quantize a pipeline result to 8-bit RGBA for direct HTML5 Canvas rendering.
pub fn quantize_rgba8(img: &ImageBuf) -> Vec<u8> {
    let mut out = Vec::with_capacity((img.width * img.height * 4) as usize);
    for chunk in img.data.chunks_exact(3) {
        out.push((f64::from(chunk[0]).clamp(0.0, 1.0) * 255.0).round_ties_even() as u8);
        out.push((f64::from(chunk[1]).clamp(0.0, 1.0) * 255.0).round_ties_even() as u8);
        out.push((f64::from(chunk[2]).clamp(0.0, 1.0) * 255.0).round_ties_even() as u8);
        out.push(255);
    }
    out
}

/// Encode 8-bit interleaved RGB as JPEG.
pub fn encode_jpeg(rgb8: &[u8], width: u32, height: u32, quality: u8) -> Result<Vec<u8>> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(
        &mut std::io::Cursor::new(&mut out),
        quality,
    )
    .write_image(rgb8, width, height, image::ExtendedColorType::Rgb8)
    .context("encoding JPEG")?;
    Ok(out)
}

/// The print's paper border. Pure #FFFFFF reads as a flat digital void beside a
/// developed frame; a fine-art matte is a hair warm and carries a faint tooth.
/// Both knobs are deliberately near the edge of perception — nudge `PAPER_TINT`
/// warmer or `GRAIN_AMP` higher for a more textured stock, cooler/0 for clinical
/// white.
pub const PAPER_TINT: [u8; 3] = [253, 251, 248];
pub const GRAIN_AMP: i16 = 3;

/// Wrap `img` in the paper border, `border_frac` of the long edge on every side.
pub fn paper_border(img: &image::RgbImage, border_frac: f32) -> image::RgbImage {
    let (w, h) = img.dimensions();
    let b = ((w.max(h) as f32) * border_frac).round() as u32;
    let mut matte = paper_matte(w + 2 * b, h + 2 * b);
    image::imageops::overlay(&mut matte, img, b as i64, b as i64);
    matte
}

/// A sheet of paper: the warm-white base plus a stable per-pixel luminance
/// grain (same delta on all channels, so the tooth is neutral, never coloured).
/// The noise is a cheap position hash — deterministic, so re-exporting a frame
/// is byte-stable and two prints of the same size share the same grain field.
fn paper_matte(w: u32, h: u32) -> image::RgbImage {
    let span = 2 * GRAIN_AMP as u32 + 1;
    image::ImageBuffer::from_fn(w, h, |x, y| {
        let mut n = x
            .wrapping_mul(374_761_393)
            .wrapping_add(y.wrapping_mul(668_265_263));
        n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
        let noise = (n % span) as i16 - GRAIN_AMP;
        let px = |c: u8| (c as i16 + noise).clamp(0, 255) as u8;
        image::Rgb([px(PAPER_TINT[0]), px(PAPER_TINT[1]), px(PAPER_TINT[2])])
    })
}
