// Rapid engine — GPU pipeline.
//
// A line-by-line translation of the per-pixel loop in rapid.rs, NOT an
// independent implementation: the CPU path stays the reference and the
// fallback, so the two must agree. Every helper below mirrors one Rust
// function of the same name, in the same order, with the same guards —
// when the CPU math changes, this changes with it or the fallback stops
// being a fallback and becomes a second look.
//
// Three WGSL-specific traps this file is deliberate about:
//   * `smoothstep` is NOT used. The builtin is undefined when edge0 >= edge1,
//     and rapid.rs legitimately calls its own smoothstep with descending
//     edges (e.g. smoothstep(35, 10, …) for the skin-tone dampener). `sstep`
//     below reproduces the Rust one, which handles that by construction.
//   * `pow` is undefined for a negative base. Every call here keeps the
//     max(0.0) guard its Rust counterpart has.
//   * arrays live in storage, not uniform, buffers — `array<f32, N>` in a
//     uniform block has a 16-byte stride, which would silently misread the
//     HSL bands and curve LUTs.

struct Params {
    width: u32,
    height: u32,
    down_w: u32,
    down_h: u32,

    has_blurred: u32,
    has_hsl: u32,
    has_color_wheels: u32,
    has_zones: u32,

    w_mult: f32,
    exposure_factor: f32,
    r_temp: f32,
    r_tint: f32,

    g_tint: f32,
    b_temp: f32,
    b_tint: f32,
    brightness_adj: f32,

    clarity: f32,
    structure: f32,
    dehaze: f32,
    contrast: f32,

    shadows: f32,
    blacks: f32,
    highlights: f32,
    saturation_adj: f32,

    vibrance: f32,
    vignette_amount: f32,
    vignette_midpoint: f32,
    vignette_roundness: f32,

    vignette_feather: f32,
    highlight_desat: f32,
    use_logc: u32,
    agx_look: u32,

    zone_shadows_exposure: f32,
    zone_shadows_contrast: f32,
    zone_shadows_saturation: f32,
    zone_midtones_exposure: f32,

    zone_midtones_contrast: f32,
    zone_midtones_saturation: f32,
    zone_highlights_exposure: f32,
    zone_highlights_contrast: f32,

    zone_highlights_saturation: f32,
    curve_luma: u32,
    curve_r: u32,
    curve_g: u32,

    curve_b: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,

    // vec4 so the trailing component pads the vec3 the CPU side stores.
    shadows_tint: vec4<f32>,
    midtones_tint: vec4<f32>,
    highlights_tint: vec4<f32>,
}

@group(0) @binding(0) var<storage, read> in_data: array<f32>;
@group(0) @binding(1) var<storage, read_write> out_data: array<f32>;
@group(0) @binding(2) var<storage, read> blurred: array<f32>;
@group(0) @binding(3) var<uniform> p: Params;
// HSL bands (8 hue, 8 sat, 8 lum) then four 256-entry curve LUTs.
@group(0) @binding(4) var<storage, read> aux: array<f32>;

const HSL_HUE: u32 = 0u;
const HSL_SAT: u32 = 8u;
const HSL_LUM: u32 = 16u;
const CURVE_BASE: u32 = 24u;
const CURVE_STRIDE: u32 = 256u;

const AGX_MIDDLE_GREY: f32 = 0.18;
const AGX_EPSILON: f32 = 1.0e-6;
const AGX_MIN_EV: f32 = -15.2;
const AGX_MAX_EV: f32 = 5.0;
const AGX_GAMMA: f32 = 2.4;
const AGX_SLOPE: f32 = 2.3843;
const AGX_TOE_POWER: f32 = 1.5;
const AGX_SHOULDER_POWER: f32 = 1.5;
const AGX_TOE_TRANSITION_X: f32 = 0.6060606;
const AGX_TOE_TRANSITION_Y: f32 = 0.43446;
const AGX_SHOULDER_TRANSITION_X: f32 = 0.6060606;
const AGX_SHOULDER_TRANSITION_Y: f32 = 0.43446;
const AGX_INTERCEPT: f32 = -1.0112;
const AGX_TOE_SCALE: f32 = -1.0359;
const AGX_SHOULDER_SCALE: f32 = 1.3475;

const LOGC3_CUT: f32 = 0.010591;
const LOGC3_A: f32 = 5.555556;
const LOGC3_B: f32 = 0.052272;
const LOGC3_C: f32 = 0.247190;
const LOGC3_D: f32 = 0.385537;
const LOGC3_E: f32 = 5.367655;
const LOGC3_F: f32 = 0.092809;

// AGX_INSET / AGX_OUTSET in rapid.rs, value for value.
fn agx_inset(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        0.5682423421 * c.r + 0.3731251307 * c.g + 0.05863252723 * c.b,
        0.1281182356 * c.r + 0.7783136252 * c.g + 0.09356813916 * c.b,
        0.07347080765 * c.r + 0.1620963122 * c.g + 0.7644328802 * c.b,
    );
}

fn agx_outset(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        1.940429221 * c.r - 0.8296109087 * c.g - 0.110818312 * c.b,
        -0.276003293 * c.r + 1.30673334 * c.g - 0.03073004751 * c.b,
        -0.1438899598 * c.r - 0.2248403189 * c.g + 1.368730279 * c.b,
    );
}

fn prophoto_to_rec709(c: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        2.0362741263 * c.r - 0.7375868484 * c.g - 0.2991716804 * c.b,
        -0.2256519640 * c.r + 1.2230755666 * c.g + 0.0027110556 * c.b,
        -0.0105510879 * c.r - 0.1348857077 * c.g + 1.1451776386 * c.b,
    );
}

/// rapid.rs::smoothstep — the builtin can't be used, see the header.
fn sstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

fn rem_euclid(a: f32, b: f32) -> f32 {
    return a - b * floor(a / b);
}

fn luma_of(c: vec3<f32>) -> f32 {
    return 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
}

/// rapid.rs::zone_weights
fn zone_weights(c: vec3<f32>) -> vec3<f32> {
    let lum_linear = max(luma_of(c), 0.0);
    let lum_norm = min(sqrt(lum_linear), 1.0);
    let shadow_w = clamp(1.0 - lum_norm * 2.0, 0.0, 1.0);
    let highlight_w = clamp((lum_norm - 0.5) * 2.0, 0.0, 1.0);
    let midtone_w = max(1.0 - shadow_w - highlight_w, 0.0);
    return vec3<f32>(shadow_w, midtone_w, highlight_w);
}

/// rapid.rs::get_blurred_luma — bilinear sample of the ÷8 guidance map.
fn blurred_luma(x: u32, y: u32) -> f32 {
    let dw = p.down_w;
    let dh = p.down_h;
    let fx = f32(x) / 8.0 - 0.5;
    let fy = f32(y) / 8.0 - 0.5;
    let x0 = u32(clamp(floor(fx), 0.0, f32(dw - 1u)));
    let x1 = min(x0 + 1u, dw - 1u);
    let y0 = u32(clamp(floor(fy), 0.0, f32(dh - 1u)));
    let y1 = min(y0 + 1u, dh - 1u);
    let tx = clamp(fx - f32(x0), 0.0, 1.0);
    let ty = clamp(fy - f32(y0), 0.0, 1.0);
    let c00 = blurred[y0 * dw + x0];
    let c10 = blurred[y0 * dw + x1];
    let c01 = blurred[y1 * dw + x0];
    let c11 = blurred[y1 * dw + x1];
    let top = c00 * (1.0 - tx) + c10 * tx;
    let bottom = c01 * (1.0 - tx) + c11 * tx;
    return top * (1.0 - ty) + bottom * ty;
}

/// rapid.rs::apply_filmic_exposure
fn apply_filmic_exposure(c: vec3<f32>, brightness_adj: f32) -> vec3<f32> {
    if (brightness_adj == 0.0) { return c; }
    let RATIONAL_CURVE_MIX = 0.95;
    let MIDTONE_STRENGTH = 1.2;
    let TOP_ANCHOR = 1.06;
    let original_luma = luma_of(c);
    if (abs(original_luma) < 0.00001) { return c; }

    let direct_adj = brightness_adj * (1.0 - RATIONAL_CURVE_MIX);
    let rational_adj = brightness_adj * RATIONAL_CURVE_MIX;
    let scale = exp2(direct_adj);
    let k = exp2(-rational_adj * MIDTONE_STRENGTH);
    let luma_abs = abs(original_luma);
    let luma_floor = floor(luma_abs / TOP_ANCHOR) * TOP_ANCHOR;
    let luma_norm = (luma_abs - luma_floor) / TOP_ANCHOR;
    let shaped_norm = luma_norm / (luma_norm + (1.0 - luma_norm) * k);
    let shaped_luma_abs = luma_floor + shaped_norm * TOP_ANCHOR;
    let new_luma = sign(original_luma) * shaped_luma_abs * scale;
    let chroma = c - vec3<f32>(original_luma);
    let total_luma_scale = new_luma / original_luma;
    let luma_weight = clamp(new_luma, 0.0, 2.0) * 0.5;
    let dynamic_exp = 0.95 - luma_weight * 0.3;
    let base_chroma_scale = pow(max(total_luma_scale, 0.0), dynamic_exp);
    let highlight_rolloff = 1.0 / (1.0 + max(new_luma - 0.9, 0.0) * 2.0);
    let chroma_scale = base_chroma_scale * highlight_rolloff;
    return vec3<f32>(new_luma) + chroma * chroma_scale;
}

/// rapid.rs::apply_local_contrast_masked (with its /100 slider rescale)
fn local_contrast_masked(c: vec3<f32>, t_blurred: f32, amount_in: f32, mask: f32) -> vec3<f32> {
    if (amount_in == 0.0 || mask < 0.001) { return c; }
    let amount = amount_in * 0.01;

    if (amount < 0.0) {
        let blur_amount = -amount * mask;
        let center_luma = max(luma_of(c), 0.0001);
        let scale = t_blurred / center_luma;
        let blurred_c = c * scale;
        return c * (1.0 - blur_amount) + blurred_c * blur_amount;
    }

    let center_luma = max(luma_of(c), 0.0);
    let safe_center = max(center_luma, 0.0001);
    let safe_blur = max(t_blurred, 0.0001);
    let log_ratio = log2(safe_center / safe_blur);
    let contrast_factor = exp2(log_ratio * amount);
    let f = c * contrast_factor;
    return c * (1.0 - mask) + f * mask;
}

/// rapid.rs::apply_local_contrast — the shadow/highlight-protected mask.
fn local_contrast(c: vec3<f32>, t_blurred: f32, amount: f32) -> vec3<f32> {
    if (amount == 0.0) { return c; }
    var mask = 1.0;
    if (amount >= 0.0) {
        let center_luma = max(luma_of(c), 0.0);
        let shadow_protection = sstep(0.0, 0.03, center_luma);
        let highlight_protection = 1.0 - sstep(0.9, 1.0, center_luma);
        mask = shadow_protection * highlight_protection;
    }
    return local_contrast_masked(c, t_blurred, amount, mask);
}

/// rapid.rs::apply_dehaze
fn apply_dehaze(c: vec3<f32>, t_blurred: f32, amount: f32) -> vec3<f32> {
    if (amount == 0.0) { return c; }
    let atmospheric = vec3<f32>(0.95, 0.97, 1.0);
    let pixel_dark = min(min(c.r, c.g), c.b);
    let regional_dark = t_blurred * 0.9;

    if (amount > 0.0) {
        let pixel_luma = max(luma_of(c), 0.0);
        let edge_diff = abs(sqrt(max(pixel_luma, 0.0)) - sqrt(max(t_blurred, 0.0)));
        let halo_protection = sstep(0.02, 0.15, edge_diff);
        let spatial_dark = regional_dark * (1.0 - halo_protection) + pixel_dark * halo_protection;
        let safe_dark = max(spatial_dark - 0.02, 0.0);
        let mapped_haze = safe_dark / (safe_dark + 0.2);
        let t = max(1.0 - amount * mapped_haze * 0.85, 0.15);

        var rec = (c - atmospheric) / t + atmospheric;
        let rec_luma = max(luma_of(rec), 0.0);
        let shadow_lift = sstep(0.1, 0.0, rec_luma) * (1.0 - t) * 0.15;
        rec = rec + vec3<f32>(shadow_lift);

        let sat_boost = (1.0 - t) * 0.5;
        let final_luma = max(luma_of(rec), 0.0);
        return max(vec3<f32>(final_luma) + (rec - vec3<f32>(final_luma)) * (1.0 + sat_boost), vec3<f32>(0.0));
    }

    let safe_dark = max(regional_dark - 0.02, 0.0);
    let mapped_depth = safe_dark / (safe_dark + 0.2);
    let depth_factor = 0.4 * (1.0 - mapped_depth) + 1.0 * mapped_depth;
    let f = abs(amount) * 0.7 * depth_factor;
    return c * (1.0 - f) + atmospheric * f;
}

fn hue_distance(h1: f32, h2: f32) -> f32 {
    let d = abs(h1 - h2) % 360.0;
    if (d > 180.0) { return 360.0 - d; }
    return d;
}

/// rapid.rs::rgb_to_hsl
fn rgb_to_hsl(c: vec3<f32>) -> vec3<f32> {
    let mx = max(max(c.r, c.g), c.b);
    let mn = min(min(c.r, c.g), c.b);
    let delta = mx - mn;
    let l = (mx + mn) / 2.0;
    if (abs(delta) < 1e-6) { return vec3<f32>(0.0, 0.0, l); }

    var s: f32;
    if (l > 0.5) { s = delta / (2.0 - mx - mn); } else { s = delta / (mx + mn); }

    var h: f32;
    if (mx == c.r) {
        var off = 0.0;
        if (c.g < c.b) { off = 6.0; }
        h = (c.g - c.b) / delta + off;
    } else if (mx == c.g) {
        h = (c.b - c.r) / delta + 2.0;
    } else {
        h = (c.r - c.g) / delta + 4.0;
    }
    return vec3<f32>(h * 60.0, s, l);
}

fn hue_to_rgb(p_: f32, q: f32, t: f32) -> f32 {
    if (t < 1.0 / 6.0) { return p_ + (q - p_) * 6.0 * t; }
    if (t < 1.0 / 2.0) { return q; }
    if (t < 2.0 / 3.0) { return p_ + (q - p_) * (2.0 / 3.0 - t) * 6.0; }
    return p_;
}

/// rapid.rs::hsl_to_rgb
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> vec3<f32> {
    if (s <= 1e-6) { return vec3<f32>(l, l, l); }
    var q: f32;
    if (l < 0.5) { q = l * (1.0 + s); } else { q = l + s - l * s; }
    let p_ = 2.0 * l - q;
    let hk = h / 360.0;
    return vec3<f32>(
        hue_to_rgb(p_, q, rem_euclid(hk + 1.0 / 3.0, 1.0)),
        hue_to_rgb(p_, q, rem_euclid(hk, 1.0)),
        hue_to_rgb(p_, q, rem_euclid(hk - 1.0 / 3.0, 1.0)),
    );
}

/// rapid.rs::apply_vibrance
fn apply_vibrance(c_in: vec3<f32>, sat_adj: f32, vib_adj: f32) -> vec3<f32> {
    let luma = luma_of(c_in);
    var c = c_in;
    if (sat_adj != 0.0) {
        let f = 1.0 + sat_adj;
        c = vec3<f32>(luma) + (c - vec3<f32>(luma)) * f;
    }
    if (vib_adj == 0.0) { return c; }

    let c_max = max(max(c.r, c.g), c.b);
    let c_min = min(min(c.r, c.g), c.b);
    let delta = c_max - c_min;
    if (delta < 0.02) { return c; }

    let current_sat = delta / max(c_max, 0.001);
    if (vib_adj > 0.0) {
        let sat_mask = 1.0 - sstep(0.4, 0.9, current_sat);
        let hsl = rgb_to_hsl(max(c, vec3<f32>(0.0)));
        var hue_dist = abs(hsl.x - 25.0);
        hue_dist = min(hue_dist, 360.0 - hue_dist);
        let is_skin = sstep(35.0, 10.0, hue_dist);
        let skin_dampener = 1.0 * (1.0 - is_skin) + 0.6 * is_skin;
        let f = 1.0 + vib_adj * sat_mask * skin_dampener * 3.0;
        return vec3<f32>(luma) + (c - vec3<f32>(luma)) * f;
    }
    let desat_mask = 1.0 - sstep(0.2, 0.8, current_sat);
    let f = 1.0 + vib_adj * desat_mask;
    return vec3<f32>(luma) + (c - vec3<f32>(luma)) * f;
}

// The argument is non-negative by construction on both branches that reach
// here (toe: negative scale with x < tx; shoulder: positive scale with
// x > tx), so the max() only guards WGSL's undefined pow() on a negative
// base — it never changes a value the CPU path would produce.
fn agx_sigmoid(x: f32, power: f32) -> f32 {
    return x / pow(1.0 + pow(max(x, 0.0), power), 1.0 / power);
}

fn agx_scaled_sigmoid(x: f32, scale: f32, slope: f32, power: f32, tx: f32, ty: f32) -> f32 {
    return scale * agx_sigmoid(slope * (x - tx) / scale, power) + ty;
}

fn agx_curve_channel(x: f32) -> f32 {
    var result: f32;
    if (x < AGX_TOE_TRANSITION_X) {
        result = agx_scaled_sigmoid(x, AGX_TOE_SCALE, AGX_SLOPE, AGX_TOE_POWER,
                                    AGX_TOE_TRANSITION_X, AGX_TOE_TRANSITION_Y);
    } else if (x <= AGX_SHOULDER_TRANSITION_X) {
        result = AGX_SLOPE * x + AGX_INTERCEPT;
    } else {
        result = agx_scaled_sigmoid(x, AGX_SHOULDER_SCALE, AGX_SLOPE, AGX_SHOULDER_POWER,
                                    AGX_SHOULDER_TRANSITION_X, AGX_SHOULDER_TRANSITION_Y);
    }
    return clamp(result, 0.0, 1.0);
}

/// rapid.rs::agx_tonemap
fn agx_tonemap(c_in: vec3<f32>) -> vec3<f32> {
    var c = c_in;
    let min_c = min(min(c.r, c.g), c.b);
    if (min_c < 0.0) { c = c - vec3<f32>(min_c); }

    let inset = agx_inset(c);
    let range_ev = AGX_MAX_EV - AGX_MIN_EV;
    let log_c = vec3<f32>(
        (log2(max(inset.r / AGX_MIDDLE_GREY, AGX_EPSILON)) - AGX_MIN_EV) / range_ev,
        (log2(max(inset.g / AGX_MIDDLE_GREY, AGX_EPSILON)) - AGX_MIN_EV) / range_ev,
        (log2(max(inset.b / AGX_MIDDLE_GREY, AGX_EPSILON)) - AGX_MIN_EV) / range_ev,
    );
    let mapped = clamp(log_c, vec3<f32>(0.0), vec3<f32>(1.0));
    let curved = vec3<f32>(
        agx_curve_channel(mapped.r),
        agx_curve_channel(mapped.g),
        agx_curve_channel(mapped.b),
    );
    let gammad = pow(max(curved, vec3<f32>(0.0)), vec3<f32>(AGX_GAMMA));
    return agx_outset(gammad);
}

fn logc3_encode(x: f32) -> f32 {
    if (x > LOGC3_CUT) {
        return LOGC3_C * (log2(LOGC3_A * x + LOGC3_B) / log2(10.0)) + LOGC3_D;
    }
    return LOGC3_E * x + LOGC3_F;
}

/// curves.rs::sample — linear interpolation between LUT entries.
fn curve_sample(slot: u32, x_in: f32) -> f32 {
    let base = CURVE_BASE + slot * CURVE_STRIDE;
    let x = clamp(x_in, 0.0, 1.0) * f32(CURVE_STRIDE - 1u);
    let i = u32(x);
    if (i >= CURVE_STRIDE - 1u) { return aux[base + CURVE_STRIDE - 1u]; }
    let f = x - f32(i);
    return aux[base + i] * (1.0 - f) + aux[base + i + 1u] * f;
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    if (x >= p.width || y >= p.height) { return; }
    let idx = (y * p.width + x) * 3u;

    // 1. Whites multiplier, exposure, temp & tint WB
    var c = vec3<f32>(
        in_data[idx] * p.w_mult * p.exposure_factor * p.r_temp * p.r_tint,
        in_data[idx + 1u] * p.w_mult * p.exposure_factor * p.g_tint,
        in_data[idx + 2u] * p.w_mult * p.exposure_factor * p.b_temp * p.b_tint,
    );

    let has_blur = p.has_blurred == 1u;

    // Clarity, structure, dehaze (guidance map)
    if ((p.clarity != 0.0 || p.structure != 0.0 || p.dehaze != 0.0) && has_blur) {
        let t_blurred = blurred_luma(x, y);
        if (p.clarity != 0.0) { c = local_contrast(c, t_blurred, p.clarity); }
        if (p.structure != 0.0) { c = local_contrast(c, t_blurred, p.structure); }
        if (p.dehaze != 0.0) { c = apply_dehaze(c, t_blurred, p.dehaze); }
    }

    // 2. Filmic exposure / brightness
    if (p.brightness_adj != 0.0) { c = apply_filmic_exposure(c, p.brightness_adj); }

    // 3. Perceptual S-curve contrast around 0.5 in 1/2.2 space
    if (abs(p.contrast) > 1e-4) {
        let g_power = 2.2;
        let strength = exp2(p.contrast * 1.25);
        var outc = c;
        for (var i = 0u; i < 3u; i = i + 1u) {
            let safe_val = max(c[i], 0.0);
            let perceptual = min(pow(safe_val, 1.0 / g_power), 1.0);
            var curved: f32;
            if (perceptual < 0.5) {
                curved = 0.5 * pow(2.0 * perceptual, strength);
            } else {
                curved = 1.0 - 0.5 * pow(2.0 * (1.0 - perceptual), strength);
            }
            let contrast_adjusted = pow(max(curved, 0.0), g_power);
            let mix_factor = sstep(1.0, 1.01, safe_val);
            outc[i] = contrast_adjusted * (1.0 - mix_factor) + safe_val * mix_factor;
        }
        c = outc;
    }

    // 4. Shadows & blacks
    if (p.shadows != 0.0 || p.blacks != 0.0) {
        let luma_linear = max(luma_of(c), 0.0);
        let safe_pixel_luma = max(luma_linear, 0.0001);
        let t_pixel = pow(safe_pixel_luma, 0.4545);
        var t_blurred = 0.0;
        if (has_blur) { t_blurred = blurred_luma(x, y); }

        let shadow_lift = p.shadows * t_pixel * pow(max(1.0 - t_pixel, 0.0), 4.5);
        let black_lift = p.blacks * t_pixel * pow(max(1.0 - t_pixel, 0.0), 12.0);
        let lift_amount = max(shadow_lift + black_lift, 0.0);
        let t_pixel_curved = max(t_pixel + shadow_lift + black_lift, 0.0);

        let shadow_pivot = 0.2;
        let stretch_factor = 1.0 + lift_amount * 1.3;
        let contrasted_t = shadow_pivot + (t_pixel_curved - shadow_pivot) * stretch_factor;
        let final_t = max(t_pixel_curved * 0.15 + contrasted_t * 0.85, 0.0);
        let curved_luma = pow(final_t, 2.2);

        let luma_ratio = curved_luma / safe_pixel_luma;
        c = c * luma_ratio;

        let detail = t_pixel / max(t_blurred, 0.0001);
        let safe_detail = clamp(detail, 0.8, 1.25);
        let noise_protection = sstep(0.0, 0.1, t_blurred);
        let detail_amp = 1.0 + lift_amount * 1.2 * noise_protection;
        let enhanced_detail = pow(safe_detail, detail_amp);
        let detail_correction = enhanced_detail / safe_detail;
        let linear_correction = pow(max(detail_correction, 0.0), 2.2);
        c = c * linear_correction;

        let final_luma_ratio = luma_ratio * linear_correction;
        if (final_luma_ratio > 1.0) {
            let recovered_luma = luma_of(c);
            let boost_amount = clamp((final_luma_ratio - 1.0) * 0.15, 0.0, 0.4);
            c = c * (1.0 - boost_amount) + vec3<f32>(recovered_luma) * boost_amount;
        }
    }

    // 5. Highlights
    if (p.highlights != 0.0) {
        let pixel_luma = max(luma_of(c), 0.0);
        let safe_pixel_luma = max(pixel_luma, 0.0001);
        let pixel_mask_input = tanh(safe_pixel_luma * 1.5);
        let highlight_mask = sstep(0.3, 0.95, pixel_mask_input);

        if (highlight_mask > 0.001) {
            var adjusted: vec3<f32>;
            if (p.highlights < 0.0) {
                var new_luma: f32;
                if (pixel_luma <= 1.0) {
                    new_luma = pow(pixel_luma, 1.0 - p.highlights * 1.75);
                } else {
                    let luma_excess = pixel_luma - 1.0;
                    let compression_strength = -p.highlights * 6.0;
                    new_luma = 1.0 + luma_excess / (1.0 + luma_excess * compression_strength);
                }
                let luma_scale = new_luma / safe_pixel_luma;
                let tonal = c * luma_scale;
                let desat = sstep(1.0, 10.0, pixel_luma);
                adjusted = tonal * (1.0 - desat) + vec3<f32>(new_luma) * desat;
            } else {
                adjusted = c * exp2(p.highlights * 1.75);
            }
            c = c * (1.0 - highlight_mask) + adjusted * highlight_mask;
        }
    }

    // 6. 3-way colour wheels
    if (p.has_color_wheels == 1u) {
        let w = zone_weights(c);
        c = c
            + p.shadows_tint.rgb * w.x * 0.2
            + p.midtones_tint.rgb * w.y * 0.2
            + p.highlights_tint.rgb * w.z * 0.2;
    }

    // 6a. Zone tone shaping
    if (p.has_zones == 1u) {
        let w = zone_weights(c);
        let blended_ev = p.zone_shadows_exposure * w.x
            + p.zone_midtones_exposure * w.y
            + p.zone_highlights_exposure * w.z;
        if (blended_ev != 0.0) { c = c * exp2(blended_ev); }

        if (has_blur) {
            let t_blurred = blurred_luma(x, y);
            if (p.zone_shadows_contrast != 0.0) {
                c = local_contrast_masked(c, t_blurred, p.zone_shadows_contrast, w.x);
            }
            if (p.zone_midtones_contrast != 0.0) {
                c = local_contrast_masked(c, t_blurred, p.zone_midtones_contrast, w.y);
            }
            if (p.zone_highlights_contrast != 0.0) {
                c = local_contrast_masked(c, t_blurred, p.zone_highlights_contrast, w.z);
            }
        }

        let blended_sat = p.zone_shadows_saturation * w.x
            + p.zone_midtones_saturation * w.y
            + p.zone_highlights_saturation * w.z;
        if (blended_sat != 0.0) {
            let luma = luma_of(c);
            let sat_factor = max(1.0 + blended_sat / 100.0, 0.0);
            c = vec3<f32>(luma) + (c - vec3<f32>(luma)) * sat_factor;
        }
    }

    // 7. Saturation, vibrance & HSL matrix
    if (abs(p.saturation_adj) > 1e-4 || p.vibrance != 0.0 || p.has_hsl == 1u) {
        let n = apply_vibrance(c, p.saturation_adj, p.vibrance);
        if (p.has_hsl == 1u) {
            let hsl = rgb_to_hsl(max(n, vec3<f32>(0.0)));
            var hue_adj = 0.0;
            var sat_adj = 0.0;
            var lum_adj = 0.0;
            for (var i = 0u; i < 8u; i = i + 1u) {
                var center = 0.0;
                switch i {
                    case 0u: { center = 0.0; }
                    case 1u: { center = 30.0; }
                    case 2u: { center = 60.0; }
                    case 3u: { center = 120.0; }
                    case 4u: { center = 180.0; }
                    case 5u: { center = 240.0; }
                    case 6u: { center = 270.0; }
                    default: { center = 300.0; }
                }
                let dist = hue_distance(hsl.x, center);
                if (dist < 45.0) {
                    let weight = max(1.0 - dist / 45.0, 0.0);
                    hue_adj = hue_adj + aux[HSL_HUE + i] * weight;
                    sat_adj = sat_adj + aux[HSL_SAT + i] * weight;
                    lum_adj = lum_adj + aux[HSL_LUM + i] * weight;
                }
            }
            let new_h = rem_euclid(hsl.x + hue_adj, 360.0);
            let new_s = clamp(hsl.y * (1.0 + sat_adj / 100.0), 0.0, 1.0);
            let new_l = max(hsl.z * (1.0 + lum_adj / 100.0), 0.0);
            c = hsl_to_rgb(new_h, new_s, new_l);
        } else {
            c = n;
        }
    }

    // 8. Vignetting
    if (p.vignette_amount != 0.0) {
        let w_f = f32(p.width);
        let h_f = f32(p.height);
        let aspect = h_f / w_f;
        let uv_x = (f32(x) / w_f - 0.5) * 2.0;
        let uv_y = (f32(y) / h_f - 0.5) * 2.0;
        let v_round = 1.0 - p.vignette_roundness;
        let v_feather = p.vignette_feather * 0.5;
        let uv_round_x = sign(uv_x) * pow(abs(uv_x), v_round);
        let uv_round_y = sign(uv_y) * pow(abs(uv_y), v_round);
        let dist = sqrt(uv_round_x * uv_round_x + uv_round_y * uv_round_y * aspect * aspect) * 0.5;
        let vignette_mask = sstep(p.vignette_midpoint - v_feather, p.vignette_midpoint + v_feather, dist);
        if (p.vignette_amount < 0.0) {
            c = c * (1.0 + p.vignette_amount * vignette_mask);
        } else {
            let amount = p.vignette_amount * vignette_mask;
            c = c * (1.0 - amount) + vec3<f32>(amount);
        }
    }

    // ProPhoto (D50) -> Rec.709 linear (D65)
    let c709 = max(prophoto_to_rec709(c), vec3<f32>(0.0));

    var outc: vec3<f32>;
    if (p.use_logc == 1u) {
        outc = vec3<f32>(
            logc3_encode(max(c709.r, 0.0)),
            logc3_encode(max(c709.g, 0.0)),
            logc3_encode(max(c709.b, 0.0)),
        );
    } else {
        var agx = max(agx_tonemap(c709), vec3<f32>(0.0));

        switch p.agx_look {
            case 1u: { agx = pow(agx, vec3<f32>(1.15)); }               // punchy
            case 2u: {                                                  // golden
                agx = vec3<f32>(
                    min(agx.r * 1.04, 1.0),
                    min(agx.g * 1.01, 1.0),
                    max(agx.b * 0.95, 0.0),
                );
            }
            case 3u: { agx = pow(agx, vec3<f32>(0.88)); }               // soft
            case 4u: { agx = vec3<f32>(luma_of(agx)); }                 // filmic b&w
            default: {}
        }

        let max_c = max(max(agx.r, agx.g), agx.b);
        if (max_c > 0.85 && p.highlight_desat > 0.0) {
            let desat_w = clamp((max_c - 0.85) / 0.15, 0.0, 1.0) * p.highlight_desat;
            let avg_c = (agx.r + agx.g + agx.b) / 3.0;
            agx = agx * (1.0 - desat_w) + vec3<f32>(avg_c) * desat_w;
        }

        // Tone curves last, on display-referred 0..1 (see rapid.rs).
        if (p.curve_luma == 1u || p.curve_r == 1u || p.curve_g == 1u || p.curve_b == 1u) {
            agx = clamp(agx, vec3<f32>(0.0), vec3<f32>(1.0));
            if (p.curve_luma == 1u) {
                agx = vec3<f32>(
                    curve_sample(0u, agx.r),
                    curve_sample(0u, agx.g),
                    curve_sample(0u, agx.b),
                );
            }
            if (p.curve_r == 1u) { agx.r = curve_sample(1u, agx.r); }
            if (p.curve_g == 1u) { agx.g = curve_sample(2u, agx.g); }
            if (p.curve_b == 1u) { agx.b = curve_sample(3u, agx.b); }
        }

        outc = clamp(agx, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    out_data[idx] = outc.r;
    out_data[idx + 1u] = outc.g;
    out_data[idx + 2u] = outc.b;
}
