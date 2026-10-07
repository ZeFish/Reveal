//! Local contrast, prepared for the film.
//!
//! A film's curve is a long, smooth S: it takes the whole scene and bends it, and what it was given
//! to bend decides how the bent picture reads. A decoded photo reaches it with all its detail laid
//! out evenly across the tonal range. This stage rearranges that, *before* the film sees it:
//!
//!   * the broad tones (the "base": what is bright and what is dark over large areas) are pressed
//!     together a little around the photo's middle, so the film's curve has more room to separate the
//!     textures sitting on them;
//!   * the textures themselves — three scales of detail, fine to coarse — are put back with a gain
//!     that depends on how clean the signal is at that brightness: full where the sensor is clean,
//!     less in the shadows where detail is mostly noise, so noise is not what gets amplified;
//!   * the darkest tones are left where they were (the black is the photo's own), and the middle of
//!     the photo does not move: exposure is preserved.
//!
//! It works on luminance in log2 (so a stop is a stop everywhere) with edge-preserving filters —
//! guided filters, so a boundary between a bright window and a dark wall does not glow — and puts the
//! new luminance back on the colour as a ratio, so hue and saturation are the photo's. At amount 0
//! nothing happens; at 1 it is the recipe worked out on the wedding photographs.
//!
//! The noise curve is the one measured on a Fujifilm X-T5 (green-channel sigma/level per stop below
//! white). It is a stand-in for "typical" and the first thing to replace with per-camera numbers.

use rayon::prelude::*;
use spektrafilm_math::image::ImageBuf;

/// ProPhoto (D50) luminance weights: the working space of the pipeline input.
const LUMA: [f32; 3] = [0.28804, 0.71187, 0.00009];
/// Darkest luminance worked with, as a stop: 2^-14 below white.
const FLOOR: f32 = 1.0 / 16384.0;

/// The three scales of detail, as a fraction of the long edge (3, 12 and 64 px on a 3864 px frame),
/// the guided filter's regularisation at each, and the gain each gets at best: (radius, eps, strength, cap).
const BANDS: [(f32, f32, f32, f32); 3] = [
    (0.0008, 0.02, 1.0, 1.0),
    (0.0031, 0.05, 1.1, 1.35),
    (0.0166, 0.25, 1.2, 1.5),
];
/// Regularisation of the guided filter that extracts the base, at its radius (a fraction of the long edge).
const BASE: (f32, f32) = (0.0166, 0.25);
/// How much of the base's distance from the middle is kept at full amount: 1.0 would change nothing.
const BASE_SLOPE: f32 = 0.72;
/// Noise, as sigma/level, at a stop below white (-1 … -12).
const NOISE: [(f32, f32); 12] = [
    (-12.0, 0.30),
    (-11.0, 0.198),
    (-10.0, 0.125),
    (-9.0, 0.089),
    (-8.0, 0.065),
    (-7.0, 0.047),
    (-6.0, 0.044),
    (-5.0, 0.040),
    (-4.0, 0.037),
    (-3.0, 0.036),
    (-2.0, 0.030),
    (-1.0, 0.015),
];

/// Pixel radius of a band on a frame of this size.
pub(crate) fn radius(long_edge: usize, fraction: f32) -> usize {
    ((long_edge as f32 * fraction).round() as usize).max(1)
}

fn noise_at(stop: f32) -> f32 {
    if stop <= NOISE[0].0 {
        return NOISE[0].1;
    }
    for pair in NOISE.windows(2) {
        let ((s0, n0), (s1, n1)) = (pair[0], pair[1]);
        if stop <= s1 {
            return n0 + (n1 - n0) * (stop - s0) / (s1 - s0);
        }
    }
    NOISE[NOISE.len() - 1].1
}

/// The gain a band gets at a brightness: what the signal can bear, between a quarter and `cap`,
/// times the band's strength.
fn band_gain(stop: f32, strength: f32, cap: f32) -> f32 {
    (0.05 / noise_at(stop).max(1e-3)).clamp(0.25, cap) * strength
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ── box and guided filters ───────────────────────────────────────────────────────────────────

/// Mean of each row over a window of 2r+1, the edges repeated.
fn box_rows(src: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; w * h];
    out.par_chunks_mut(w).zip(src.par_chunks(w)).for_each(|(dst, row)| {
        let n = 2 * r + 1;
        let at = |i: isize| row[i.clamp(0, w as isize - 1) as usize] as f64;
        let mut sum: f64 = (-(r as isize)..=(r as isize)).map(at).sum();
        for x in 0..w {
            dst[x] = (sum / n as f64) as f32;
            sum += at(x as isize + r as isize + 1) - at(x as isize - r as isize);
        }
    });
    out
}

fn transpose(src: &[f32], w: usize, h: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; w * h];
    out.par_chunks_mut(h).enumerate().for_each(|(x, col)| {
        for y in 0..h {
            col[y] = src[y * w + x];
        }
    });
    out
}

/// Mean over a (2r+1) × (2r+1) window, the edges repeated.
fn box_mean(src: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let horizontal = box_rows(src, w, h, r);
    let vertical = box_rows(&transpose(&horizontal, w, h), h, w, r);
    transpose(&vertical, h, w)
}

/// Edge-preserving smoothing of `src` guided by itself: flat areas are averaged, a strong edge is kept.
fn guided(src: &[f32], w: usize, h: usize, r: usize, eps: f32) -> Vec<f32> {
    let mean = box_mean(src, w, h, r);
    let squares: Vec<f32> = src.par_iter().map(|v| v * v).collect();
    let mean_sq = box_mean(&squares, w, h, r);
    let (a, b): (Vec<f32>, Vec<f32>) = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let var = (mean_sq[i] - mean[i] * mean[i]).max(0.0);
            let a = var / (var + eps);
            (a, mean[i] - a * mean[i])
        })
        .unzip();
    let (mean_a, mean_b) = (box_mean(&a, w, h, r), box_mean(&b, w, h, r));
    (0..w * h).into_par_iter().map(|i| mean_a[i] * src[i] + mean_b[i]).collect()
}

/// A percentile of a large array, from a sample of it.
fn percentile(values: &[f32], p: f32) -> f32 {
    let step = (values.len() / 100_000).max(1);
    let mut sample: Vec<f32> = values.iter().step_by(step).copied().collect();
    sample.sort_by(|a, b| a.total_cmp(b));
    sample[((sample.len() - 1) as f32 * p).round() as usize]
}

// ── the stage ────────────────────────────────────────────────────────────────────────────────

/// The image as the film should see it. `amount` is 0 (untouched) to 1 (the full recipe).
pub fn prepare(img: &ImageBuf, amount: f32) -> ImageBuf {
    let amount = amount.clamp(0.0, 1.0);
    let (w, h) = (img.width as usize, img.height as usize);
    if amount <= 0.0 || w == 0 || h == 0 {
        return img.clone();
    }
    let long_edge = w.max(h);

    let luminance: Vec<f32> = img
        .data
        .par_chunks_exact(3)
        .map(|p| (p[0] * LUMA[0] + p[1] * LUMA[1] + p[2] * LUMA[2]).max(FLOOR))
        .collect();
    let log: Vec<f32> = luminance.par_iter().map(|y| y.log2()).collect();

    // Three scales of detail, and the base under them.
    let r = |f: f32| radius(long_edge, f);
    let fine = guided(&log, w, h, r(BANDS[0].0), BANDS[0].1);
    let medium = guided(&log, w, h, r(BANDS[1].0), BANDS[1].1);
    let base = guided(&log, w, h, r(BASE.0), BASE.1);

    let pivot = percentile(&base, 0.5);
    let black = percentile(&log, 0.001);
    // The base is pressed together only where the photo is well above its own black.
    let squeeze = amount * (1.0 - BASE_SLOPE);

    let data: Vec<f32> = img
        .data
        .par_chunks_exact(3)
        .enumerate()
        .flat_map_iter(|(i, p)| {
            let (d1, d2, d3) = (log[i] - fine[i], fine[i] - medium[i], medium[i] - base[i]);
            let stop = log[i];
            let gain = |(_, _, strength, cap): (f32, f32, f32, f32)| {
                1.0 + amount * (band_gain(stop, strength, cap) - 1.0)
            };
            let slope = 1.0 - squeeze * smoothstep(black, black + 4.0, base[i]);
            let new_log = pivot + (base[i] - pivot) * slope
                + gain(BANDS[2]) * d3
                + gain(BANDS[1]) * d2
                + gain(BANDS[0]) * d1;
            let ratio = (new_log.exp2() / luminance[i]).clamp(0.25, 8.0);
            [p[0] * ratio, p[1] * ratio, p[2] * ratio]
        })
        .collect();
    ImageBuf::from_data(img.width, img.height, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grey frame whose luminance is `f(x, y)` (scene-linear, white = 1).
    fn frame(w: usize, h: usize, f: impl Fn(usize, usize) -> f32) -> ImageBuf {
        let mut data = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                let v = f(x, y);
                data.extend_from_slice(&[v, v, v]);
            }
        }
        ImageBuf::from_data(w as u32, h as u32, data)
    }

    fn lum(img: &ImageBuf, i: usize) -> f32 {
        img.data[i * 3]
    }

    /// Standard deviation of log2 luminance over a window of the frame.
    fn texture(img: &ImageBuf, x0: usize, x1: usize, y0: usize, y1: usize) -> f32 {
        let w = img.width as usize;
        let v: Vec<f32> = (y0..y1).flat_map(|y| (x0..x1).map(move |x| (x, y))).map(|(x, y)| lum(img, y * w + x).max(FLOOR).log2()).collect();
        let m = v.iter().sum::<f32>() / v.len() as f32;
        (v.iter().map(|a| (a - m).powi(2)).sum::<f32>() / v.len() as f32).sqrt()
    }

    #[test]
    fn at_zero_nothing_happens() {
        let img = frame(40, 30, |x, y| 0.05 + 0.01 * ((x + y) % 7) as f32);
        assert_eq!(prepare(&img, 0.0).data, img.data);
    }

    #[test]
    fn a_flat_picture_comes_out_flat_and_where_it_was() {
        let img = frame(64, 48, |_, _| 0.18);
        let out = prepare(&img, 1.0);
        assert!(out.data.iter().all(|v| (v - 0.18).abs() < 1e-4), "no texture, nothing to rearrange");
    }

    #[test]
    fn the_filters_keep_a_step_edge_and_smooth_the_noise_beside_it() {
        let (w, h) = (96, 32);
        let step: Vec<f32> = (0..w * h).map(|i| if (i % w) < w / 2 { 0.0 } else { 4.0 }).collect();
        let smoothed = guided(&step, w, h, 3, 0.02);
        // Far from the edge the plateaus are what they were; at the edge the step is still a step.
        assert!(smoothed[10].abs() < 0.05 && (smoothed[w - 10] - 4.0).abs() < 0.05);
        assert!(smoothed[w / 2 + 2] - smoothed[w / 2 - 3] > 3.5, "an edge guided filter does not blur away");
    }

    #[test]
    fn the_box_mean_of_a_ramp_is_the_ramp_away_from_the_edges() {
        let (w, h) = (40, 8);
        let ramp: Vec<f32> = (0..w * h).map(|i| (i % w) as f32).collect();
        let m = box_mean(&ramp, w, h, 3);
        assert!((m[20] - 20.0).abs() < 1e-4);
        assert!((m[w + 20] - 20.0).abs() < 1e-4);
    }

    #[test]
    fn texture_in_the_clean_midtones_is_strengthened() {
        // A mid-grey wall (about -3 stops) with a coarse texture of +/- 0.4 stop on it.
        let img = frame(160, 120, |x, y| 0.125 * (0.4 * ((x / 8 + y / 8) % 2) as f32 * 2.0 - 0.4).exp2());
        let out = prepare(&img, 1.0);
        let (before, after) = (texture(&img, 30, 130, 20, 100), texture(&out, 30, 130, 20, 100));
        assert!(after > before * 1.15, "texture {before:.3} -> {after:.3} stops");
    }

    #[test]
    fn noise_deep_in_the_shadows_is_not_amplified() {
        // The same texture, -10 stops below white, where it is mostly noise.
        let img = frame(160, 120, |x, y| (1.0 / 1024.0) * (0.4 * ((x / 8 + y / 8) % 2) as f32 * 2.0 - 0.4).exp2());
        let out = prepare(&img, 1.0);
        let (before, after) = (texture(&img, 30, 130, 20, 100), texture(&out, 30, 130, 20, 100));
        assert!(after < before * 1.05, "shadow texture {before:.3} -> {after:.3} stops");
    }

    #[test]
    fn the_middle_of_the_picture_does_not_move() {
        // A gradient with texture: the median luminance is the pivot, so exposure is kept.
        let img = frame(160, 120, |x, y| 0.02 + 0.5 * (x as f32 / 160.0).powi(2) + 0.01 * ((x / 6 + y / 6) % 2) as f32);
        let out = prepare(&img, 1.0);
        let median = |i: &ImageBuf| {
            let mut v: Vec<f32> = (0..(i.width * i.height) as usize).map(|k| lum(i, k)).collect();
            v.sort_by(|a, b| a.total_cmp(b));
            v[v.len() / 2]
        };
        let (a, b) = (median(&img), median(&out));
        assert!((b / a - 1.0).abs() < 0.08, "median {a:.4} -> {b:.4}");
    }

    #[test]
    fn the_black_stays_where_it_was() {
        // A photo with a real black (0.1 percentile) and a bright field: the black is not lifted.
        let img = frame(200, 100, |x, y| if y < 10 { 0.0004 } else { 0.05 + 0.6 * (x as f32 / 200.0) });
        let out = prepare(&img, 1.0);
        let darkest = |i: &ImageBuf| (0..5 * 200).map(|k| lum(i, k)).fold(f32::MAX, f32::min);
        assert!(darkest(&out) < darkest(&img) * 1.5, "black {:.5} -> {:.5}", darkest(&img), darkest(&out));
    }

    #[test]
    fn colour_is_the_photos_own() {
        let (w, h) = (80, 60);
        let mut data = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let v = 0.1 + 0.05 * ((x / 5 + y / 5) % 2) as f32;
                data.extend_from_slice(&[v * 1.4, v, v * 0.6]);
            }
        }
        let img = ImageBuf::from_data(w as u32, h as u32, data);
        let out = prepare(&img, 1.0);
        for p in out.data.chunks_exact(3) {
            assert!((p[0] / p[1] - 1.4).abs() < 1e-3 && (p[2] / p[1] - 0.6).abs() < 1e-3, "ratios kept");
        }
    }

    #[test]
    fn a_smaller_amount_is_a_smaller_change() {
        let img = frame(160, 120, |x, y| 0.125 * (0.4 * ((x / 8 + y / 8) % 2) as f32 * 2.0 - 0.4).exp2());
        let t = |a: f32| texture(&prepare(&img, a), 30, 130, 20, 100);
        let (t0, t5, t1) = (texture(&img, 30, 130, 20, 100), t(0.5), t(1.0));
        assert!(t0 < t5 && t5 < t1, "{t0:.3} < {t5:.3} < {t1:.3}");
    }

    #[test]
    fn the_scales_follow_the_size_of_the_frame() {
        // 3, 12 and 64 px on a 3864 px frame; the same share of a smaller or larger one.
        assert_eq!([0.0008, 0.0031, 0.0166].map(|f| radius(3864, f)), [3, 12, 64]);
        assert_eq!(radius(768, 0.0166), 13);
        assert_eq!(radius(100, 0.0008), 1, "never below one pixel");
    }

    #[test]
    fn nothing_comes_out_broken() {
        let img = frame(120, 90, |x, y| if (x + y) % 13 == 0 { 0.0 } else { 3.0 * (x as f32 / 120.0) });
        let out = prepare(&img, 1.0);
        assert!(out.data.iter().all(|v| v.is_finite() && *v >= 0.0));
    }
}
