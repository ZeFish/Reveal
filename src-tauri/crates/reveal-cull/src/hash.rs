//! Cheap per-photo signals computed on an already-decoded grayscale preview:
//! a blur metric (variance of Laplacian), an exposure-clipping fraction, and
//! a difference hash (dHash) for near-duplicate/burst clustering. None of
//! this touches a RAW decode or the GPU — it's meant to run on hundreds of
//! photos in well under a second combined.

use image::{imageops::FilterType, GrayImage};

/// Downscale to at most `max_edge` on the long side (no-op if already
/// smaller) — the metrics below don't need full resolution, and shrinking
/// first keeps the per-photo cost flat regardless of the source preview size.
pub fn resize_gray(gray: &GrayImage, max_edge: u32) -> GrayImage {
    let (w, h) = gray.dimensions();
    if w <= max_edge && h <= max_edge {
        return gray.clone();
    }
    let scale = max_edge as f32 / w.max(h) as f32;
    let nw = ((w as f32) * scale).round().max(1.0) as u32;
    let nh = ((h as f32) * scale).round().max(1.0) as u32;
    image::imageops::resize(gray, nw, nh, FilterType::Triangle)
}

/// Variance of the Laplacian response — a standard cheap focus/blur metric.
/// Higher = sharper. Motion blur and out-of-focus frames both collapse this
/// toward zero because a smoothed image has little high-frequency response.
pub fn sharpness_variance(gray: &GrayImage) -> f64 {
    let (w, h) = gray.dimensions();
    if w < 3 || h < 3 {
        return 0.0;
    }
    let mut responses = Vec::with_capacity(((w - 2) * (h - 2)) as usize);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let center = gray.get_pixel(x, y)[0] as f64;
            let up = gray.get_pixel(x, y - 1)[0] as f64;
            let down = gray.get_pixel(x, y + 1)[0] as f64;
            let left = gray.get_pixel(x - 1, y)[0] as f64;
            let right = gray.get_pixel(x + 1, y)[0] as f64;
            responses.push(up + down + left + right - 4.0 * center);
        }
    }
    variance(&responses)
}

fn variance(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64
}

/// Fraction of pixels clamped at the dark/light extremes — a cheap proxy for
/// blown highlights or blocked shadows. 0 = nothing clipped, 1 = every pixel.
pub fn exposure_penalty(gray: &GrayImage) -> f64 {
    let total = gray.len() as u64;
    if total == 0 {
        return 1.0;
    }
    let mut clipped = 0u64;
    for p in gray.pixels() {
        let v = p[0];
        if v <= 2 || v >= 253 {
            clipped += 1;
        }
    }
    clipped as f64 / total as f64
}

/// A 64-bit difference hash (dHash): resize to 9x8, compare each pixel to its
/// right neighbor. Near-duplicate/burst frames land close in Hamming
/// distance; unrelated frames don't.
pub fn dhash(gray: &GrayImage) -> u64 {
    let small = image::imageops::resize(gray, 9, 8, FilterType::Triangle);
    let mut hash: u64 = 0;
    for y in 0..8u32 {
        for x in 0..8u32 {
            let left = small.get_pixel(x, y)[0];
            let right = small.get_pixel(x + 1, y)[0];
            hash <<= 1;
            if left > right {
                hash |= 1;
            }
        }
    }
    hash
}

pub fn hamming(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Luma;

    fn checkerboard(size: u32, block: u32) -> GrayImage {
        GrayImage::from_fn(size, size, |x, y| {
            let on = ((x / block) + (y / block)) % 2 == 0;
            Luma([if on { 235u8 } else { 20u8 }])
        })
    }

    fn solid(size: u32, value: u8) -> GrayImage {
        GrayImage::from_pixel(size, size, Luma([value]))
    }

    #[test]
    fn sharp_image_scores_higher_than_blurred() {
        let sharp = checkerboard(64, 4);
        let blurred = image::imageops::blur(&sharp, 3.0);
        assert!(
            sharpness_variance(&sharp) > sharpness_variance(&blurred) * 2.0,
            "a heavily blurred checkerboard should score much lower than the sharp original"
        );
    }

    #[test]
    fn flat_image_has_zero_sharpness() {
        let flat = solid(32, 128);
        assert_eq!(sharpness_variance(&flat), 0.0);
    }

    #[test]
    fn blown_out_image_is_fully_penalized() {
        let white = solid(16, 255);
        assert_eq!(exposure_penalty(&white), 1.0);
    }

    #[test]
    fn midtone_image_has_no_exposure_penalty() {
        let mid = solid(16, 128);
        assert_eq!(exposure_penalty(&mid), 0.0);
    }

    #[test]
    fn dhash_is_stable_under_light_blur_but_differs_across_scenes() {
        let a = checkerboard(64, 4);
        let a_blurred = image::imageops::blur(&a, 1.0);
        let different_scene = checkerboard(64, 16); // same pattern, very different block size/edges

        let near_duplicate_distance = hamming(dhash(&a), dhash(&a_blurred));
        let different_scene_distance = hamming(dhash(&a), dhash(&different_scene));

        assert!(
            near_duplicate_distance < different_scene_distance,
            "a lightly blurred near-duplicate ({near_duplicate_distance}) should hash closer than a differently framed scene ({different_scene_distance})"
        );
    }
}
