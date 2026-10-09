use spektrafilm_math::image::ImageBuf;
use crate::recipe::Recipe;

/// Below this the picture is not turned at all (a slider resting at 0 must cost nothing and
/// change nothing).
pub const STRAIGHTEN_EPSILON_DEG: f32 = 0.01;

/// Box-filter downscale in linear light so the long edge is `max_px`.
pub fn downscale(img: &ImageBuf, max_px: u32) -> ImageBuf {
    use rayon::prelude::*;
    let (w, h) = (img.width as usize, img.height as usize);
    let scale = max_px as f64 / img.width.max(img.height) as f64;
    let nw = ((img.width as f64 * scale).round() as usize).max(1);
    let nh = ((img.height as f64 * scale).round() as usize).max(1);

    let mut data = vec![spektrafilm_math::precision::from_f32(0.0); nw * nh * 3];
    data.par_chunks_mut(nw * 3)
        .enumerate()
        .for_each(|(oy, row)| {
            let y0 = oy * h / nh;
            let y1 = (((oy + 1) * h) / nh).max(y0 + 1).min(h);
            let src = &img.data;
            for ox in 0..nw {
                let x0 = ox * w / nw;
                let x1 = (((ox + 1) * w) / nw).max(x0 + 1).min(w);
                let mut acc0 = 0.0f64;
                let mut acc1 = 0.0f64;
                let mut acc2 = 0.0f64;
                let mut n = 0.0f64;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let i = (y * w + x) * 3;
                        acc0 += f64::from(src[i]);
                        acc1 += f64::from(src[i + 1]);
                        acc2 += f64::from(src[i + 2]);
                        n += 1.0;
                    }
                }
                let inv_n = (1.0 / n) as f32;
                let out_idx = ox * 3;
                row[out_idx] = spektrafilm_math::precision::from_f32((acc0 as f32) * inv_n);
                row[out_idx + 1] = spektrafilm_math::precision::from_f32((acc1 as f32) * inv_n);
                row[out_idx + 2] = spektrafilm_math::precision::from_f32((acc2 as f32) * inv_n);
            }
        });

    ImageBuf::from_data(nw as u32, nh as u32, data)
}

/// Apply straighten (`crop_angle`), crop coordinates (normalized 0..1) and flip_h / flip_v to
/// an ImageBuf.
///
/// The geometry is Lightroom's, and the one the Crop tab draws: the picture is turned by
/// `crop_angle` degrees (clockwise, positive) about the centre of the whole frame; the crop
/// rectangle is axis-aligned in that turned frame, which keeps the size of the original; the
/// flips come last, on the cropped result. So a source pixel `q` (from the centre) lands at
/// `R(angle)·q` in the frame, and an output pixel reads the source at `R(-angle)` of its place.
/// Where the turned picture does not reach (the corners of a crop pushed past it) the nearest
/// edge pixel is repeated, rather than black — the Crop tab keeps the frame inside, so this only
/// matters for a recipe edited by hand.
pub fn crop_and_flip(img: &ImageBuf, recipe: &Recipe) -> ImageBuf {
    let (src_w, src_h) = (img.width as usize, img.height as usize);
    if src_w == 0 || src_h == 0 {
        return img.clone();
    }

    let needs_crop = recipe.crop_w > 0.0
        && recipe.crop_h > 0.0
        && (recipe.crop_w < 0.999 || recipe.crop_h < 0.999 || recipe.crop_x > 0.001 || recipe.crop_y > 0.001);
    let needs_flip = recipe.flip_h || recipe.flip_v;
    let needs_turn = recipe.crop_angle.abs() >= STRAIGHTEN_EPSILON_DEG;

    if !needs_crop && !needs_flip && !needs_turn {
        return img.clone();
    }

    let cx = (recipe.crop_x.clamp(0.0, 1.0) * src_w as f32).floor() as usize;
    let cy = (recipe.crop_y.clamp(0.0, 1.0) * src_h as f32).floor() as usize;
    let cw = (recipe.crop_w.clamp(0.01, 1.0) * src_w as f32).round() as usize;
    let ch = (recipe.crop_h.clamp(0.01, 1.0) * src_h as f32).round() as usize;

    let x0 = cx.min(src_w.saturating_sub(1));
    let y0 = cy.min(src_h.saturating_sub(1));
    let x1 = (x0 + cw).min(src_w).max(x0 + 1);
    let y1 = (y0 + ch).min(src_h).max(y0 + 1);
    let out_w = x1 - x0;
    let out_h = y1 - y0;

    if needs_turn {
        return turn_and_crop(img, recipe, (x0, y0, out_w, out_h));
    }

    let mut out_data = Vec::with_capacity(out_w * out_h * 3);
    for out_y in 0..out_h {
        let src_y = if recipe.flip_v { y1 - 1 - out_y } else { y0 + out_y };
        for out_x in 0..out_w {
            let src_x = if recipe.flip_h { x1 - 1 - out_x } else { x0 + out_x };
            let idx = (src_y * src_w + src_x) * 3;
            if idx + 2 < img.data.len() {
                out_data.push(img.data[idx]);
                out_data.push(img.data[idx + 1]);
                out_data.push(img.data[idx + 2]);
            } else {
                out_data.extend_from_slice(&[0.0, 0.0, 0.0]);
            }
        }
    }

    ImageBuf::from_data(out_w as u32, out_h as u32, out_data)
}

/// The straightened path of [`crop_and_flip`]: `rect` is the crop in pixels of the (turned)
/// frame — `(x, y, width, height)`.
fn turn_and_crop(img: &ImageBuf, recipe: &Recipe, rect: (usize, usize, usize, usize)) -> ImageBuf {
    use rayon::prelude::*;

    let (src_w, src_h) = (img.width as usize, img.height as usize);
    let (x0, y0, out_w, out_h) = rect;
    let (half_w, half_h) = (src_w as f32 / 2.0, src_h as f32 / 2.0);
    let theta = recipe.crop_angle.to_radians();
    let (sin, cos) = theta.sin_cos();
    let (flip_h, flip_v) = (recipe.flip_h, recipe.flip_v);

    // One row per task: a 24-megapixel crop is a few thousand independent rows.
    let rows: Vec<Vec<f32>> = (0..out_h)
        .into_par_iter()
        .map(|out_y| {
            let frame_y = if flip_v { y0 + out_h - 1 - out_y } else { y0 + out_y };
            let dy = frame_y as f32 + 0.5 - half_h;
            let mut row = Vec::with_capacity(out_w * 3);
            for out_x in 0..out_w {
                let frame_x = if flip_h { x0 + out_w - 1 - out_x } else { x0 + out_x };
                let dx = frame_x as f32 + 0.5 - half_w;
                // R(-angle) of the frame point, back to a source pixel centre.
                let sx = dx * cos + dy * sin + half_w - 0.5;
                let sy = -dx * sin + dy * cos + half_h - 0.5;
                row.extend_from_slice(&sample_bilinear(img, src_w, src_h, sx, sy));
            }
            row
        })
        .collect();

    ImageBuf::from_data(out_w as u32, out_h as u32, rows.into_iter().flatten().collect())
}

/// Bilinear sample at pixel-centre coordinates `(x, y)`, repeating the edge outside the image.
fn sample_bilinear(img: &ImageBuf, w: usize, h: usize, x: f32, y: f32) -> [f32; 3] {
    let x = x.clamp(0.0, (w - 1) as f32);
    let y = y.clamp(0.0, (h - 1) as f32);
    let (ix, iy) = (x.floor() as usize, y.floor() as usize);
    let (jx, jy) = ((ix + 1).min(w - 1), (iy + 1).min(h - 1));
    let (fx, fy) = (x - ix as f32, y - iy as f32);
    let at = |px: usize, py: usize| -> [f32; 3] {
        let i = (py * w + px) * 3;
        [img.data[i], img.data[i + 1], img.data[i + 2]]
    };
    let (a, b, c, d) = (at(ix, iy), at(jx, iy), at(ix, jy), at(jx, jy));
    let mut out = [0.0; 3];
    for k in 0..3 {
        let top = a[k] + (b[k] - a[k]) * fx;
        let bottom = c[k] + (d[k] - c[k]) * fx;
        out[k] = top + (bottom - top) * fy;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_and_flip_reduces_dimensions_and_inverts() {
        let input = ImageBuf::from_data(100, 100, vec![1.0f32; 100 * 100 * 3]);
        let mut recipe = Recipe::default();
        recipe.crop_x = 0.25;
        recipe.crop_y = 0.25;
        recipe.crop_w = 0.5;
        recipe.crop_h = 0.5;
        recipe.flip_h = true;

        let cropped = crop_and_flip(&input, &recipe);
        assert_eq!(cropped.width, 50);
        assert_eq!(cropped.height, 50);
        assert_eq!(cropped.data.len(), 50 * 50 * 3);
    }

    // ── straighten ──

    /// An N×N image, grey 0 everywhere except one white pixel at (px, py).
    fn dot(n: usize, px: usize, py: usize) -> ImageBuf {
        let mut data = vec![0.0f32; n * n * 3];
        let i = (py * n + px) * 3;
        data[i..i + 3].copy_from_slice(&[1.0, 1.0, 1.0]);
        ImageBuf::from_data(n as u32, n as u32, data)
    }

    fn white_at(img: &ImageBuf) -> Option<(usize, usize)> {
        let w = img.width as usize;
        img.data.chunks_exact(3).position(|p| p[0] > 0.5).map(|i| (i % w, i / w))
    }

    #[test]
    fn straighten_zero_changes_nothing() {
        let input = dot(9, 2, 3);
        let out = crop_and_flip(&input, &Recipe::default());
        assert_eq!(out.data, input.data);
    }

    #[test]
    fn a_quarter_turn_is_clockwise() {
        // Top-left goes to top-right when the picture is turned clockwise.
        let mut recipe = Recipe::default();
        recipe.crop_angle = 90.0;
        let out = crop_and_flip(&dot(9, 0, 0), &recipe);
        assert_eq!((out.width, out.height), (9, 9));
        assert_eq!(white_at(&out), Some((8, 0)));
    }

    #[test]
    fn a_negative_angle_turns_the_other_way() {
        let mut recipe = Recipe::default();
        recipe.crop_angle = -90.0;
        let out = crop_and_flip(&dot(9, 0, 0), &recipe);
        assert_eq!(white_at(&out), Some((0, 8)));
    }

    #[test]
    fn straightening_keeps_the_size_of_the_frame_it_was_cropped_from() {
        let input = ImageBuf::from_data(40, 30, vec![0.5f32; 40 * 30 * 3]);
        let mut recipe = Recipe::default();
        recipe.crop_angle = 7.0;
        recipe.crop_x = 0.25;
        recipe.crop_y = 0.25;
        recipe.crop_w = 0.5;
        recipe.crop_h = 0.5;
        let out = crop_and_flip(&input, &recipe);
        assert_eq!((out.width, out.height), (20, 15));
    }

    #[test]
    fn a_turned_flat_picture_stays_flat_with_no_black_corners() {
        // The whole frame, turned: the corners the picture does not reach repeat its edge.
        let input = ImageBuf::from_data(32, 24, vec![0.5f32; 32 * 24 * 3]);
        let mut recipe = Recipe::default();
        recipe.crop_angle = 20.0;
        let out = crop_and_flip(&input, &recipe);
        assert!(out.data.iter().all(|v| (v - 0.5).abs() < 1e-4));
    }

    #[test]
    fn a_small_turn_moves_a_pixel_by_the_expected_amount() {
        // 5° clockwise about the centre of a 101-pixel frame: the point 40 px right of the
        // centre lands at (50 + 40·cos5°, 50 + 40·sin5°) ≈ (89.85, 53.49). Bilinear sampling
        // spreads a lone pixel over its neighbours, so find it by its centre of gravity.
        let mut recipe = Recipe::default();
        recipe.crop_angle = 5.0;
        let out = crop_and_flip(&dot(101, 90, 50), &recipe);
        let w = out.width as usize;
        let (mut sum, mut gx, mut gy) = (0.0f32, 0.0f32, 0.0f32);
        for (i, p) in out.data.chunks_exact(3).enumerate() {
            sum += p[0];
            gx += p[0] * (i % w) as f32;
            gy += p[0] * (i / w) as f32;
        }
        assert!(sum > 0.5, "the dot survives");
        assert!((gx / sum - 89.85).abs() < 0.5, "x = {}", gx / sum);
        assert!((gy / sum - 53.49).abs() < 0.5, "y = {}", gy / sum);
    }

    #[test]
    fn the_flip_comes_after_the_turn_and_the_crop() {
        let mut turned = Recipe::default();
        turned.crop_angle = 90.0;
        let plain = crop_and_flip(&dot(9, 0, 0), &turned);
        let mut mirrored = turned.clone();
        mirrored.flip_h = true;
        let flipped = crop_and_flip(&dot(9, 0, 0), &mirrored);
        let (x, y) = white_at(&plain).unwrap();
        assert_eq!(white_at(&flipped), Some((8 - x, y)));
    }
}
