//! RAW decoding: `RawDecoder` trait, rawler backend.
//!
//! Contract: `decode_linear` produces **scene-linear RGB, sRGB primaries,
//! interleaved f32, no transfer curve, UNCLIPPED** — negative components are
//! preserved. They encode colors outside the sRGB gamut (saturated foliage,
//! warm highlights); flooring them here shifts hue exactly the way the Swift
//! app once suffered before it moved its working space to Rec2020. The
//! engine converts to the pipeline's wide working space (ProPhoto RGB, the
//! Python reference's input space) BEFORE any clamp, so the gamut survives
//! the handoff.

use std::path::Path;

mod libraw;
pub use libraw::{
    capture_dimensions, capture_header, capture_timestamp, oriented_dimensions, extract_thumb_jpeg, extract_thumb_preview,
    LibrawDecoder, ThumbPreview,
};

/// Extensions the library treats as camera RAW files.
pub const RAW_EXTENSIONS: &[&str] = &[
    "raf", "dng", "nef", "arw", "cr2", "cr3", "orf", "rw2", "pef", "srw",
];

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The primaries a decoder emitted. The engine converts to its working
/// space (ProPhoto RGB) with the matching matrix.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Primaries {
    /// Linear sRGB/Rec.709 primaries, D65 (rawler's `Calibrate` output).
    SRgbLinear,
    /// ACES2065-1 (AP0), the Python reference's libraw output space.
    Aces2065_1,
}

/// Scene-linear RGB image, interleaved f32, unclipped.
pub struct LinearImage {
    pub width: u32,
    pub height: u32,
    /// RGB interleaved, row-major, len == width * height * 3.
    pub data: Vec<f32>,
    pub primaries: Primaries,
}

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("RAW decode failed: {0}")]
    Decode(String),
    #[error("RAW develop failed: {0}")]
    Develop(String),
}

pub trait RawDecoder: Send + Sync {
    /// Decode to scene-linear RGB. `fast` requests a half-resolution decode
    /// (skips the expensive full demosaic — Fuji X-Trans Markesteijn is ~20 s
    /// at full res) for previews; the export path passes `false` for full
    /// quality.
    fn decode_linear(&self, path: &Path, fast: bool) -> Result<LinearImage, DecodeError>;
}

/// Dynamic Decoder Registry — resolves the optimal decoder slice for a file.
pub struct DecoderRegistry;

impl RawDecoder for DecoderRegistry {
    fn decode_linear(&self, path: &Path, fast: bool) -> Result<LinearImage, DecodeError> {
        Self::get_decoder(path).decode_linear(path, fast)
    }
}

impl DecoderRegistry {
    pub fn get_decoder(path: &Path) -> Box<dyn RawDecoder> {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if RAW_EXTENSIONS.contains(&ext.as_str()) {
            Box::new(LibrawDecoder)
        } else if ["jpg", "jpeg", "png", "tif", "tiff"].contains(&ext.as_str()) {
            Box::new(RasterDecoder)
        } else {
            Box::new(RawlerDecoder)
        }
    }
}

/// Rendered sRGB sources have a transfer curve, unlike sensor data. PhotoKit
/// converts wide-gamut/HEIC inputs to oriented sRGB TIFF before this boundary.
pub struct RasterDecoder;

impl RawDecoder for RasterDecoder {
    fn decode_linear(&self, path: &Path, _fast: bool) -> Result<LinearImage, DecodeError> {
        use image::ImageDecoder;
        let reader = image::ImageReader::open(path).map_err(|e| DecodeError::Decode(e.to_string()))?;
        let mut decoder = reader.into_decoder().map_err(|e| DecodeError::Decode(e.to_string()))?;
        let orientation = decoder.orientation().map_err(|e| DecodeError::Decode(e.to_string()))?;
        let mut image = image::DynamicImage::from_decoder(decoder).map_err(|e| DecodeError::Decode(e.to_string()))?;
        image.apply_orientation(orientation);
        let rgb = image.to_rgb32f();
        Ok(LinearImage {
            width: rgb.width(), height: rgb.height(),
            data: rgb.into_raw().into_iter().map(srgb_to_linear).collect(),
            primaries: Primaries::SRgbLinear,
        })
    }
}

fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) }
}

#[cfg(test)]
mod raster_tests {
    use super::*;

    #[test]
    fn rendered_inputs_are_linearized_before_development() {
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert!((srgb_to_linear(1.0) - 1.0).abs() < 1e-6);
        assert!((srgb_to_linear(0.5) - 0.21404114).abs() < 1e-6);
        assert!((srgb_to_linear(0.04045) - 0.0031308).abs() < 1e-7);
    }
}

/// The rawler backend — pure Rust, same decoder spektrafilm-rs uses.
pub struct RawlerDecoder;

impl RawDecoder for RawlerDecoder {
    fn decode_linear(&self, path: &Path, _fast: bool) -> Result<LinearImage, DecodeError> {
        // rawler has no cheap half-res path; it's the non-RAW fallback anyway,
        // so `fast` is a no-op here (callers downscale afterwards).
        use rawler::imgop::develop::{Intermediate, ProcessingStep, RawDevelop};
        use rayon::prelude::*;

        let raw = rawler::decode_file(path).map_err(|e| DecodeError::Decode(format!("{e:?}")))?;

        let mut dev = RawDevelop::default();
        // Drop the sRGB gamma step: we hand scene-linear to the film
        // pipeline, which applies its own sRGB OETF at the very end.
        dev.steps.retain(|s| !matches!(s, ProcessingStep::SRgb));

        let intermediate = dev
            .develop_intermediate(&raw)
            .map_err(|e| DecodeError::Develop(format!("{e:?}")))?;

        // No clamp here — out-of-gamut negatives carry real color (see the
        // module contract above).
        let (width, height, data) = match intermediate {
            Intermediate::Monochrome(px) => {
                let data: Vec<f32> = px
                    .data
                    .par_iter()
                    .flat_map_iter(|&v| [v, v, v])
                    .collect();
                (px.width as u32, px.height as u32, data)
            }
            Intermediate::ThreeColor(px) => {
                let data: Vec<f32> = px
                    .data
                    .par_iter()
                    .flat_map_iter(|p| [p[0], p[1], p[2]])
                    .collect();
                (px.width as u32, px.height as u32, data)
            }
            Intermediate::FourColor(px) => {
                let data: Vec<f32> = px
                    .data
                    .par_iter()
                    .flat_map_iter(|p| [p[0], p[1], p[2]])
                    .collect();
                (px.width as u32, px.height as u32, data)
            }
        };

        Ok(LinearImage {
            width,
            height,
            data,
            primaries: Primaries::SRgbLinear,
        })
    }
}

impl LinearImage {
    /// Box-filter downscale so the long edge is at most `max_px`.
    /// Averaging in linear light is the physically correct downscale, and
    /// it keeps preview renders cheap without touching the full-res path.
    pub fn downscale_to(&self, max_px: u32) -> LinearImage {
        let long = self.width.max(self.height);
        if long <= max_px || max_px == 0 {
            return LinearImage {
                width: self.width,
                height: self.height,
                data: self.data.clone(),
                primaries: self.primaries,
            };
        }
        let scale = max_px as f64 / long as f64;
        let nw = ((self.width as f64 * scale).round() as u32).max(1);
        let nh = ((self.height as f64 * scale).round() as u32).max(1);

        use rayon::prelude::*;
        let (w, h) = (self.width as usize, self.height as usize);
        let data: Vec<f32> = (0..nh as usize)
            .into_par_iter()
            .flat_map_iter(|oy| {
                let y0 = oy * h / nh as usize;
                let y1 = (((oy + 1) * h) / nh as usize).max(y0 + 1).min(h);
                let src = &self.data;
                (0..nw as usize).flat_map(move |ox| {
                    let x0 = ox * w / nw as usize;
                    let x1 = (((ox + 1) * w) / nw as usize).max(x0 + 1).min(w);
                    let mut acc = [0.0f64; 3];
                    let mut n = 0.0f64;
                    for y in y0..y1 {
                        for x in x0..x1 {
                            let i = (y * w + x) * 3;
                            acc[0] += src[i] as f64;
                            acc[1] += src[i + 1] as f64;
                            acc[2] += src[i + 2] as f64;
                            n += 1.0;
                        }
                    }
                    [
                        (acc[0] / n) as f32,
                        (acc[1] / n) as f32,
                        (acc[2] / n) as f32,
                    ]
                })
            })
            .collect();

        LinearImage {
            width: nw,
            height: nh,
            data,
            primaries: self.primaries,
        }
    }
}
