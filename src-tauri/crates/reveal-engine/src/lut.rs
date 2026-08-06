//! Minimal 3D LUT (.cube) support — parse + trilinear sample + opacity blend.
//!
//! Format: the Adobe/DaVinci `.cube` spec (widely supported — any grading
//! tool exports these). Only 3D LUTs; 1D `.cube` files are rejected with a
//! clear error rather than silently misreading them as 3D.

use std::path::Path;

use anyhow::{bail, Context, Result};
use rayon::prelude::*;
use spektrafilm_math::image::ImageBuf;

pub struct Cube {
    size: usize,
    /// Flattened red-fastest, then green, then blue (the .cube row order).
    data: Vec<[f32; 3]>,
    domain_min: [f32; 3],
    domain_max: [f32; 3],
}

impl Cube {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading LUT {}", path.display()))?;
        let mut size = 0usize;
        let mut domain_min = [0.0f32; 3];
        let mut domain_max = [1.0f32; 3];
        let mut data = Vec::new();
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("LUT_3D_SIZE") {
                size = rest.trim().parse().context("parsing LUT_3D_SIZE")?;
                data.reserve(size * size * size);
                continue;
            }
            if line.starts_with("LUT_1D_SIZE") {
                bail!(
                    "{}: 1D .cube LUTs aren't supported, only 3D",
                    path.display()
                );
            }
            if line.starts_with("TITLE") {
                continue;
            }
            if let Some(rest) = line.strip_prefix("DOMAIN_MIN") {
                domain_min = parse_triplet(rest)?;
                continue;
            }
            if let Some(rest) = line.strip_prefix("DOMAIN_MAX") {
                domain_max = parse_triplet(rest)?;
                continue;
            }
            // Anything else is a data row: three floats.
            data.push(parse_triplet(line)?);
        }
        anyhow::ensure!(
            size >= 2,
            "{}: missing or invalid LUT_3D_SIZE",
            path.display()
        );
        anyhow::ensure!(
            data.len() == size * size * size,
            "{}: {} data rows, expected {size}³ = {}",
            path.display(),
            data.len(),
            size * size * size,
        );
        Ok(Self {
            size,
            data,
            domain_min,
            domain_max,
        })
    }

    /// Trilinear-interpolated sample at `rgb` (0..1, any domain handled via
    /// DOMAIN_MIN/MAX).
    fn sample(&self, rgb: [f32; 3]) -> [f32; 3] {
        let n = self.size;
        let nf = (n - 1) as f32;
        let mut idx = [0usize; 3];
        let mut frac = [0.0f32; 3];
        for c in 0..3 {
            let d0 = self.domain_min[c];
            let d1 = self.domain_max[c].max(d0 + 1e-6);
            let t = ((rgb[c] - d0) / (d1 - d0)).clamp(0.0, 1.0) * nf;
            idx[c] = (t.floor() as usize).min(n.saturating_sub(2));
            frac[c] = t - idx[c] as f32;
        }
        let at = |r: usize, g: usize, b: usize| -> [f32; 3] { self.data[r + g * n + b * n * n] };
        let (r0, g0, b0) = (idx[0], idx[1], idx[2]);
        let (r1, g1, b1) = (r0 + 1, g0 + 1, b0 + 1);
        let (fr, fg, fb) = (frac[0], frac[1], frac[2]);
        let lerp3 = |a: [f32; 3], b: [f32; 3], t: f32| {
            [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ]
        };
        let c00 = lerp3(at(r0, g0, b0), at(r1, g0, b0), fr);
        let c10 = lerp3(at(r0, g1, b0), at(r1, g1, b0), fr);
        let c01 = lerp3(at(r0, g0, b1), at(r1, g0, b1), fr);
        let c11 = lerp3(at(r0, g1, b1), at(r1, g1, b1), fr);
        let c0 = lerp3(c00, c10, fg);
        let c1 = lerp3(c01, c11, fg);
        lerp3(c0, c1, fb)
    }
}

fn parse_triplet(s: &str) -> Result<[f32; 3]> {
    let parts: Vec<f32> = s
        .split_whitespace()
        .map(|p| p.parse::<f32>())
        .collect::<Result<_, _>>()
        .context("parsing LUT numeric row")?;
    anyhow::ensure!(parts.len() == 3, "expected 3 values, got {}", parts.len());
    Ok([parts[0], parts[1], parts[2]])
}

fn blend(rgb: [f32; 3], cube: &Cube, opacity: f32) -> [f32; 3] {
    if opacity <= 0.0 {
        return rgb;
    }
    let out = cube.sample(rgb);
    let t = opacity.clamp(0.0, 1.0);
    [
        rgb[0] + (out[0] - rgb[0]) * t,
        rgb[1] + (out[1] - rgb[1]) * t,
        rgb[2] + (out[2] - rgb[2]) * t,
    ]
}

fn linear_to_srgb(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x <= 0.0031308 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

fn srgb_to_linear(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

/// ProPhoto RGB (linear, D50) → sRGB/Rec.709 (linear, D65). Exact inverse of
/// the pipeline's `SRGB_TO_PROPHOTO` (colour-science CAT02), reused from
/// `rapid.rs`. The pre-LUT round-trip needs the *primaries* change as well as
/// the EOTF — the old code applied `linear_to_srgb` straight to ProPhoto-linear
/// values, which is a primaries mismatch that shifted midtones and hue.
const PROPHOTO_TO_SRGB: [[f32; 3]; 3] = [
    [2.0362741263, -0.7375868484, -0.2991716804],
    [-0.2256519640, 1.2230755666, 0.0027110556],
    [-0.0105510879, -0.1348857077, 1.1451776386],
];
const SRGB_TO_PROPHOTO: [[f32; 3]; 3] = [
    [0.5288241004, 0.3340609866, 0.1373616909],
    [0.0975294148, 0.8790074094, 0.0233981175],
    [0.0163599018, 0.1066124933, 0.8772485185],
];

fn mul3(m: &[[f32; 3]; 3], r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    (
        m[0][0] * r + m[0][1] * g + m[0][2] * b,
        m[1][0] * r + m[1][1] * g + m[1][2] * b,
        m[2][0] * r + m[2][1] * g + m[2][2] * b,
    )
}

/// Apply a stack of LUTs to a buffer that's ALREADY display-encoded sRGB
/// (the film pipeline's output) — no colour-space round trip needed. Used
/// for the "after" stack, a finishing pass on the graded image.
pub fn apply_stack_display(img: &mut ImageBuf, stack: &[(std::sync::Arc<Cube>, f32)]) {
    if stack.is_empty() {
        return;
    }
    img.par_pixels_mut().for_each(|px| {
        let mut rgb = [px[0], px[1], px[2]];
        for (cube, opacity) in stack {
            rgb = blend(rgb, cube, *opacity);
        }
        px[0] = rgb[0];
        px[1] = rgb[1];
        px[2] = rgb[2];
    });
}

/// Apply a stack of LUTs to a scene-linear **ProPhoto** buffer (the pipeline's
/// working space). Most `.cube` LUTs are authored for display-encoded sRGB
/// footage, so each sample round-trips ProPhoto-linear → sRGB-linear (primaries)
/// → sRGB-EOTF → LUT → inverse EOTF → inverse primaries → ProPhoto-linear.
/// Used for the "before" stack, a creative pre-grade ahead of the simulation.
pub fn apply_stack_linear(img: &mut ImageBuf, stack: &[(std::sync::Arc<Cube>, f32)]) {
    if stack.is_empty() {
        return;
    }
    img.par_pixels_mut().for_each(|px| {
        // ProPhoto-linear → sRGB-linear (primaries), then sRGB-encode.
        let (sr, sg, sb) = mul3(&PROPHOTO_TO_SRGB, px[0], px[1], px[2]);
        let mut rgb = [linear_to_srgb(sr), linear_to_srgb(sg), linear_to_srgb(sb)];
        for (cube, opacity) in stack {
            rgb = blend(rgb, cube, *opacity);
        }
        // sRGB-decode, then sRGB-linear → ProPhoto-linear (inverse primaries).
        let (lr, lg, lb) = (
            srgb_to_linear(rgb[0]),
            srgb_to_linear(rgb[1]),
            srgb_to_linear(rgb[2]),
        );
        let (pr, pg, pb) = mul3(&SRGB_TO_PROPHOTO, lr, lg, lb);
        px[0] = pr;
        px[1] = pg;
        px[2] = pb;
    });
}
