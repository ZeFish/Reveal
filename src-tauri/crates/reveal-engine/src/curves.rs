//! Tone curves — control points in, a 256-entry lookup table out.
//!
//! Interpolation is monotone cubic Hermite (Fritsch–Carlson), not the
//! Catmull-Rom a curve editor usually reaches for first. The difference
//! matters here: Catmull-Rom overshoots around an uneven point spacing, and
//! an overshoot in a TONE curve is not a cosmetic wobble — it's a segment
//! where raising the input LOWERS the output, so a gradient inverts and a
//! highlight rolls backwards into a dark band. Fritsch–Carlson clamps the
//! tangents to keep the curve monotone wherever the points are.

/// Resolution of the generated lookup table. 256 entries with linear
/// interpolation between them is well under 8-bit quantization for any
/// curve shape a human can drag.
pub const LUT_SIZE: usize = 256;

/// A curve that does nothing, so callers can skip the whole stage.
pub const IDENTITY: [[f32; 2]; 2] = [[0.0, 0.0], [1.0, 1.0]];

/// True when these points describe the identity curve (the default state of
/// every channel) — lets the renderer skip building and sampling a LUT for
/// the overwhelmingly common "no curve set" case.
pub fn is_identity(points: &[[f32; 2]]) -> bool {
    if points.len() != 2 {
        return false;
    }
    const EPS: f32 = 1e-4;
    (points[0][0]).abs() < EPS
        && (points[0][1]).abs() < EPS
        && (points[1][0] - 1.0).abs() < EPS
        && (points[1][1] - 1.0).abs() < EPS
}

/// Build a 0..1 → 0..1 lookup table from control points.
///
/// Points may arrive in any order and with duplicate x values (a curve
/// editor dragging one point past another); they're sorted and de-duplicated
/// first. Fewer than two usable points means there's no curve to build.
pub fn build_lut(points: &[[f32; 2]]) -> Option<Vec<f32>> {
    let mut pts: Vec<[f32; 2]> = points
        .iter()
        .filter(|p| p[0].is_finite() && p[1].is_finite())
        .map(|p| [p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0)])
        .collect();
    pts.sort_by(|a, b| a[0].partial_cmp(&b[0]).unwrap_or(std::cmp::Ordering::Equal));
    pts.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-6);
    if pts.len() < 2 {
        return None;
    }

    let n = pts.len();
    let xs: Vec<f32> = pts.iter().map(|p| p[0]).collect();
    let ys: Vec<f32> = pts.iter().map(|p| p[1]).collect();

    // Secant slopes between consecutive points.
    let mut delta = vec![0.0f32; n - 1];
    for i in 0..n - 1 {
        delta[i] = (ys[i + 1] - ys[i]) / (xs[i + 1] - xs[i]);
    }

    // Initial tangents: the average of the two adjacent secants, one-sided
    // at the ends.
    let mut m = vec![0.0f32; n];
    m[0] = delta[0];
    m[n - 1] = delta[n - 2];
    for i in 1..n - 1 {
        m[i] = (delta[i - 1] + delta[i]) * 0.5;
    }

    // Fritsch–Carlson: flatten across any extremum, and keep each tangent
    // inside a circle of radius 3 around its secants. This is the step that
    // makes the result monotone.
    for i in 0..n - 1 {
        if delta[i].abs() < 1e-9 {
            m[i] = 0.0;
            m[i + 1] = 0.0;
            continue;
        }
        let a = m[i] / delta[i];
        let b = m[i + 1] / delta[i];
        let s = a * a + b * b;
        if s > 9.0 {
            let t = 3.0 / s.sqrt();
            m[i] = t * a * delta[i];
            m[i + 1] = t * b * delta[i];
        }
    }

    let mut lut = vec![0.0f32; LUT_SIZE];
    let mut seg = 0usize;
    for (i, slot) in lut.iter_mut().enumerate() {
        let x = i as f32 / (LUT_SIZE - 1) as f32;
        // Outside the point range the curve holds its end value, the same
        // way a curve editor's endpoints clamp.
        if x <= xs[0] {
            *slot = ys[0];
            continue;
        }
        if x >= xs[n - 1] {
            *slot = ys[n - 1];
            continue;
        }
        while seg + 2 < n && x > xs[seg + 1] {
            seg += 1;
        }
        let h = xs[seg + 1] - xs[seg];
        let t = (x - xs[seg]) / h;
        let t2 = t * t;
        let t3 = t2 * t;
        // Hermite basis.
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;
        *slot = (h00 * ys[seg] + h10 * h * m[seg] + h01 * ys[seg + 1] + h11 * h * m[seg + 1])
            .clamp(0.0, 1.0);
    }
    Some(lut)
}

/// Sample a LUT at `x` (0..1) with linear interpolation between entries.
#[inline]
pub fn sample(lut: &[f32], x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0) * (LUT_SIZE - 1) as f32;
    let i = x as usize;
    if i >= LUT_SIZE - 1 {
        return lut[LUT_SIZE - 1];
    }
    let f = x - i as f32;
    lut[i] * (1.0 - f) + lut[i + 1] * f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_points_are_recognized() {
        assert!(is_identity(&IDENTITY));
        assert!(!is_identity(&[[0.0, 0.0], [0.5, 0.7], [1.0, 1.0]]));
        assert!(!is_identity(&[[0.0, 0.2], [1.0, 1.0]]));
    }

    #[test]
    fn identity_curve_maps_input_to_itself() {
        let lut = build_lut(&IDENTITY).unwrap();
        for i in 0..=10 {
            let x = i as f32 / 10.0;
            assert!((sample(&lut, x) - x).abs() < 1e-3, "x={x} -> {}", sample(&lut, x));
        }
    }

    #[test]
    fn passes_through_its_control_points() {
        let pts = [[0.0, 0.0], [0.25, 0.1], [0.75, 0.9], [1.0, 1.0]];
        let lut = build_lut(&pts).unwrap();
        for p in pts {
            assert!(
                (sample(&lut, p[0]) - p[1]).abs() < 5e-3,
                "curve should pass through {p:?}, got {}",
                sample(&lut, p[0])
            );
        }
    }

    /// The whole reason for Fritsch–Carlson over Catmull-Rom: a steep step
    /// between close points makes a plain spline overshoot, and a tone curve
    /// that dips backwards inverts a gradient on screen.
    #[test]
    fn stays_monotone_through_a_steep_step() {
        let lut = build_lut(&[[0.0, 0.0], [0.45, 0.05], [0.55, 0.95], [1.0, 1.0]]).unwrap();
        for i in 1..LUT_SIZE {
            assert!(
                lut[i] >= lut[i - 1] - 1e-6,
                "curve went backwards at {i}: {} -> {}",
                lut[i - 1],
                lut[i]
            );
        }
    }

    #[test]
    fn output_never_leaves_the_unit_range() {
        let lut = build_lut(&[[0.0, 0.0], [0.2, 0.9], [0.8, 0.1], [1.0, 1.0]]).unwrap();
        for v in &lut {
            assert!((0.0..=1.0).contains(v), "out of range: {v}");
        }
    }

    #[test]
    fn unsorted_and_duplicate_points_are_tolerated() {
        // A curve editor can hand over points mid-drag in any order, with one
        // sitting exactly on top of another.
        let lut = build_lut(&[[1.0, 1.0], [0.5, 0.6], [0.5, 0.4], [0.0, 0.0]]).unwrap();
        assert!((sample(&lut, 0.0) - 0.0).abs() < 1e-3);
        assert!((sample(&lut, 1.0) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn too_few_points_builds_nothing() {
        assert!(build_lut(&[]).is_none());
        assert!(build_lut(&[[0.5, 0.5]]).is_none());
        // Two points at the same x collapse to one usable point.
        assert!(build_lut(&[[0.5, 0.2], [0.5, 0.8]]).is_none());
    }

    #[test]
    fn holds_end_values_outside_the_point_range() {
        let lut = build_lut(&[[0.25, 0.3], [0.75, 0.8]]).unwrap();
        assert!((sample(&lut, 0.0) - 0.3).abs() < 1e-3);
        assert!((sample(&lut, 1.0) - 0.8).abs() < 1e-3);
    }
}
