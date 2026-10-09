use reveal_decode::Primaries;

/// Decoder primaries → linear ProPhoto RGB (the pipeline's working space,
/// same as the Python loader's output). Matrices computed with
/// colour-science `matrix_RGB_to_RGB(…, "ProPhoto RGB", CAT02)` — the exact
/// conversion `load_and_process_raw_file` applies. Inputs may hold
/// out-of-gamut negatives; the clamp happens here, AFTER the gamut widens,
/// where almost nothing real is negative anymore.
pub const SRGB_TO_PROPHOTO: [[f32; 3]; 3] = [
    [0.5288241004, 0.3340609866, 0.1373616909],
    [0.0975294148, 0.8790074094, 0.0233981175],
    [0.0163599018, 0.1066124933, 0.8772485185],
];

pub const ACES_TO_PROPHOTO: [[f32; 3]; 3] = [
    [1.2393803418, -0.1639678228, -0.0752333838],
    [0.0036113619, 1.0896136492, -0.0932657921],
    [-0.0020596793, -0.0022515883, 1.0045855773],
];

pub fn to_prophoto(mut data: Vec<f32>, primaries: Primaries) -> Vec<f32> {
    let m = match primaries {
        Primaries::SRgbLinear => &SRGB_TO_PROPHOTO,
        Primaries::Aces2065_1 => &ACES_TO_PROPHOTO,
    };
    for px in data.chunks_exact_mut(3) {
        let (r, g, b) = (px[0], px[1], px[2]);
        px[0] = (m[0][0] * r + m[0][1] * g + m[0][2] * b).max(0.0);
        px[1] = (m[1][0] * r + m[1][1] * g + m[1][2] * b).max(0.0);
        px[2] = (m[2][0] * r + m[2][1] * g + m[2][2] * b).max(0.0);
    }
    data
}
