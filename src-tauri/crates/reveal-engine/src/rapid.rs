//! Rapid Engine: High-performance digital RAW processing pipeline.
//!
//! Provides parametric exposure, contrast, white balance (temperature/tint),
//! tone curves (highlights/shadows/whites/blacks), 8-channel HSL matrix shifts,
//! 3-way color wheels (shadows/midtones/highlights), linear-to-display AgX tone mapping,
//! SilverGrain film simulation, and Rapid-specific pre/post LUT support.

use anyhow::Result;
use rayon::prelude::*;
use spektrafilm_math::image::ImageBuf;
use std::path::Path;
use std::sync::Arc;

use crate::traits::{
    ControlGroup, CurveChannel, EngineControl, MixerBand, MixerChannel, MixerField, RenderEngine,
};
use crate::{Cube, LutLayer, Recipe};

pub struct RapidEngine;

impl RenderEngine for RapidEngine {
    fn id(&self) -> &'static str {
        "rapid"
    }

    fn label(&self) -> &'static str {
        "Rapid"
    }

    fn control_groups(&self) -> Vec<ControlGroup> {
        vec![
            ControlGroup {
                label: "Input (LUT & Encoding)".to_string(),
                controls: vec![
                    EngineControl::Toggle {
                        id: "use_logc".to_string(),
                        label: "LogC (cinematic)".to_string(),
                    },
                    EngineControl::LutStack {
                        stage: "pre".to_string(),
                        label: "Pre-Lut".to_string(),
                    },
                    EngineControl::Select {
                        id: "agx_look".to_string(),
                        label: "Look AgX".to_string(),
                        options_type: "agx_looks".to_string(),
                    },
                ],
            },
            ControlGroup {
                label: "Exposure & Contrast".to_string(),
                controls: vec![
                    EngineControl::Slider {
                        id: "exposure_ev".to_string(),
                        label: "Exposure".to_string(),
                        min: -3.0,
                        max: 3.0,
                        step: 0.05,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "brightness".to_string(),
                        label: "Brightness".to_string(),
                        min: -50.0,
                        max: 50.0,
                        step: 0.5,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "contrast".to_string(),
                        label: "Contrast".to_string(),
                        min: -0.5,
                        max: 0.5,
                        step: 0.01,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "saturation".to_string(),
                        label: "Saturation".to_string(),
                        min: -1.0,
                        max: 0.5,
                        step: 0.01,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "vibrance".to_string(),
                        label: "Vibrance".to_string(),
                        min: -50.0,
                        max: 50.0,
                        step: 0.5,
                        preset: false,
                    },
                ],
            },
            ControlGroup {
                label: "White Balance".to_string(),
                controls: vec![
                    EngineControl::Slider {
                        id: "temperature".to_string(),
                        label: "Temperature".to_string(),
                        min: -100.0,
                        max: 100.0,
                        step: 0.5,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "tint".to_string(),
                        label: "Tint".to_string(),
                        min: -50.0,
                        max: 50.0,
                        step: 0.5,
                        preset: false,
                    },
                ],
            },
            ControlGroup {
                label: "Tones".to_string(),
                controls: vec![
                    EngineControl::Slider {
                        id: "highlights".to_string(),
                        label: "Highlights".to_string(),
                        min: -50.0,
                        max: 50.0,
                        step: 0.5,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "whites".to_string(),
                        label: "Whites".to_string(),
                        min: -50.0,
                        max: 50.0,
                        step: 0.5,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "midtones".to_string(),
                        label: "Midtones".to_string(),
                        min: -50.0,
                        max: 50.0,
                        step: 0.5,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "shadows".to_string(),
                        label: "Shadows".to_string(),
                        min: -50.0,
                        max: 50.0,
                        step: 0.5,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "blacks".to_string(),
                        label: "Blacks".to_string(),
                        min: -50.0,
                        max: 50.0,
                        step: 0.5,
                        preset: false,
                    },
                ],
            },
            ControlGroup {
                label: "Tone Curve".to_string(),
                controls: vec![EngineControl::Curve {
                    id: "tone_curve".to_string(),
                    label: "Tone Curve".to_string(),
                    channels: vec![
                        CurveChannel {
                            id: "curve_luma".to_string(),
                            label: "Luma".to_string(),
                            color: "--color-foreground".to_string(),
                        },
                        CurveChannel {
                            id: "curve_r".to_string(),
                            label: "Red".to_string(),
                            color: "--color-red".to_string(),
                        },
                        CurveChannel {
                            id: "curve_g".to_string(),
                            label: "Green".to_string(),
                            color: "--color-green".to_string(),
                        },
                        CurveChannel {
                            id: "curve_b".to_string(),
                            label: "Blue".to_string(),
                            color: "--color-blue".to_string(),
                        },
                    ],
                }],
            },
            ControlGroup {
                label: "Hue, Saturation, Luminance".to_string(),
                controls: hsl_band_controls(),
            },
            ControlGroup {
                label: "Clarity & Haze".to_string(),
                controls: vec![
                    EngineControl::Slider {
                        id: "clarity".to_string(),
                        label: "Clarity".to_string(),
                        min: -40.0,
                        max: 60.0,
                        step: 0.5,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "structure".to_string(),
                        label: "Structure".to_string(),
                        min: -30.0,
                        max: 50.0,
                        step: 0.5,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "dehaze".to_string(),
                        label: "Dehaze".to_string(),
                        min: -30.0,
                        max: 50.0,
                        step: 0.5,
                        preset: false,
                    },
                ],
            },
            ControlGroup {
                label: "Local Tone".to_string(),
                controls: zone_tone_controls(),
            },
            ControlGroup {
                label: "Digital Effects".to_string(),
                controls: vec![
                    EngineControl::Slider {
                        id: "vignette_amount".to_string(),
                        label: "Vignette".to_string(),
                        min: -0.8,
                        max: 0.4,
                        step: 0.01,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "vignette_midpoint".to_string(),
                        label: "Vignette Midpoint".to_string(),
                        min: 0.1,
                        max: 0.9,
                        step: 0.01,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "vignette_roundness".to_string(),
                        label: "Vignette Roundness".to_string(),
                        min: 0.1,
                        max: 0.9,
                        step: 0.01,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "vignette_feather".to_string(),
                        label: "Vignette Feather".to_string(),
                        min: 0.1,
                        max: 0.9,
                        step: 0.01,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "highlight_desat".to_string(),
                        label: "Highlight Desaturation".to_string(),
                        min: 0.0,
                        max: 1.0,
                        step: 0.05,
                        preset: false,
                    },
                ],
            },
            ControlGroup {
                label: "Film Grain".to_string(),
                controls: vec![
                    EngineControl::Slider {
                        id: "grain_amount".to_string(),
                        label: "Intensity".to_string(),
                        min: 0.0,
                        max: 1.0,
                        step: 0.05,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "grain_roughness".to_string(),
                        label: "Roughness".to_string(),
                        min: 0.05,
                        max: 0.3,
                        step: 0.02,
                        preset: false,
                    },
                ],
            },
            ControlGroup {
                label: "Post-Lut".to_string(),
                controls: vec![EngineControl::LutStack {
                    stage: "post".to_string(),
                    label: "Post-Lut".to_string(),
                }],
            },
        ]
    }

    fn render(&self, input: &ImageBuf, recipe: &Recipe, luts_dir: &Path) -> Result<ImageBuf> {
        Ok(develop_rapid(input, recipe, luts_dir))
    }
}

/// Color bands for 8-channel HSL matrix (in degrees 0..360):
/// 0: Red (0° / 360°)
/// 1: Orange (30°)
/// 2: Yellow (60°)
/// 3: Green (120°)
/// 4: Aqua / Cyan (180°)
/// 5: Blue (240°)
/// 6: Purple (270°)
/// 7: Magenta (300°)
/// The angle each band sits at **as this engine measures hue** — i.e. on
/// linear ProPhoto values, which is what `rgb_to_hsl` is handed here.
///
/// These used to be 0/30/60/120/180/240/270/300: the hues those colours have
/// in gamma-encoded sRGB, which is neither the space nor the encoding the
/// matching runs in. Two mismatches stacked, and they moved the bands by up
/// to 18° — Green sat at 120 while foliage actually lands at 102, so the
/// "Green" slider was reaching past the greens toward yellow.
///
/// Each value is the hue an sRGB colour of that name has once taken to linear
/// ProPhoto. Six are exact: desaturating in HSV adds grey, grey maps to grey,
/// and adding grey leaves HSL hue untouched, so a primary or secondary keeps
/// its angle at any saturation. Orange and Purple lie between primaries and
/// do drift (Orange spans 26–42° from full saturation down); they take
/// the median.
///
/// Worth knowing before tuning these: real subject matter is not a named
/// colour plus grey. Measured foliage lands near 95° and a daylight sky near
/// 237°, both between two bands — which is what a hue mixer is for. The old
/// centre for Blue (240) happened to sit closer to real sky than 248 does,
/// but by coincidence, not design: it was two unrelated errors partly
/// cancelling. These centres are defined, not fitted.
const HUE_CENTERS: [f32; 8] = [9.5, 36.8, 68.0, 102.3, 189.5, 248.0, 259.9, 282.3];

/// English labels for `HUE_CENTERS`, in the same order — drives the HSL
/// control group so the UI never hardcodes a second copy of the band list.
const HUE_BAND_LABELS: [&str; 8] = [
    "Red", "Orange", "Yellow", "Green", "Aqua", "Blue", "Purple", "Magenta",
];

/// Swatch colour for each hue band's selector chip. These are the band's own
/// hue at a legible lightness — they're a target, not a sample of the photo.
const HUE_BAND_SWATCHES: [&str; 8] = [
    "#f87171", "#fb923c", "#facc15", "#4ade80", "#2dd4bf", "#60a5fa", "#a78bfa", "#f472b6",
];

/// The HSL matrix as one band mixer: pick a hue band, adjust its three
/// channels. Each band indexes the same position in `hsl_hue`/`hsl_sat`/
/// `hsl_lum`, so the 24 values behind it are unchanged — only the way you
/// reach them is.
fn hsl_band_controls() -> Vec<EngineControl> {
    let bands = HUE_BAND_LABELS
        .iter()
        .enumerate()
        .map(|(i, name)| MixerBand {
            label: (*name).to_string(),
            swatch: Some(HUE_BAND_SWATCHES[i].to_string()),
            fields: vec![
                MixerField { id: "hsl_hue".to_string(), index: Some(i) },
                MixerField { id: "hsl_sat".to_string(), index: Some(i) },
                MixerField { id: "hsl_lum".to_string(), index: Some(i) },
            ],
        })
        .collect();

    vec![EngineControl::BandMixer {
        label: "Color Mixer".to_string(),
        bands,
        channels: vec![
            MixerChannel { label: "Hue".to_string(), min: -45.0, max: 45.0, step: 1.0 },
            MixerChannel { label: "Saturation".to_string(), min: -100.0, max: 100.0, step: 1.0 },
            MixerChannel { label: "Luminance".to_string(), min: -100.0, max: 100.0, step: 1.0 },
        ],
    }]
}

/// Zone tone shaping (Exposure/Contrast/Saturation × Shadows/Midtones/
/// Highlights) as the same band mixer as the colour bands above: pick a zone,
/// adjust its three channels. Nine sliders in a 3×3 grid of cramped tracks
/// was no more aimable than the flat column it replaced; one interaction for
/// both sections is also one thing to learn. The zone names match the
/// unrelated tone-recovery group's labels on purpose — same vocabulary — and
/// the group title ("Local Tone") is what tells them apart.
fn zone_tone_controls() -> Vec<EngineControl> {
    const ZONES: [(&str, &str); 3] = [
        ("zone_shadows", "Shadows"),
        ("zone_midtones", "Midtones"),
        ("zone_highlights", "Highlights"),
    ];
    let bands = ZONES
        .iter()
        .map(|(id_prefix, label)| MixerBand {
            label: (*label).to_string(),
            swatch: None,
            fields: vec![
                MixerField { id: format!("{id_prefix}_exposure"), index: None },
                MixerField { id: format!("{id_prefix}_contrast"), index: None },
                MixerField { id: format!("{id_prefix}_saturation"), index: None },
            ],
        })
        .collect();

    vec![EngineControl::BandMixer {
        label: "Zone".to_string(),
        bands,
        channels: vec![
            MixerChannel { label: "Exposure".to_string(), min: -2.0, max: 2.0, step: 0.05 },
            MixerChannel { label: "Contrast".to_string(), min: -50.0, max: 50.0, step: 0.5 },
            MixerChannel { label: "Saturation".to_string(), min: -100.0, max: 100.0, step: 1.0 },
        ],
    }]
}

/// Kelvin the temperature slider means at each end. The UI already labels the
/// slider in Kelvin with exactly this mapping (EngineRunner's `formatVal`),
/// so this makes the number on screen the number the maths uses.
const WB_BASE_KELVIN: f32 = 5500.0;
const WB_COOL_PER_UNIT: f32 = 35.0; // slider -100 -> 2000 K
const WB_WARM_PER_UNIT: f32 = 45.0; // slider +100 -> 10000 K

/// xy chromaticity of a blackbody at `kelvin` (Kim et al.'s cubic fit to the
/// Planckian locus, valid 1667–25000 K).
fn planckian_xy(kelvin: f32) -> (f32, f32) {
    let t = 1000.0 / kelvin.clamp(1667.0, 25000.0);
    let x = if kelvin <= 4000.0 {
        -0.2661239 * t * t * t - 0.2343589 * t * t + 0.8776956 * t + 0.179910
    } else {
        -3.0258469 * t * t * t + 2.1070379 * t * t + 0.2226347 * t + 0.240390
    };
    let y = if kelvin <= 2222.0 {
        -1.1063814 * x * x * x - 1.34811020 * x * x + 2.18555832 * x - 0.20219683
    } else if kelvin <= 4000.0 {
        -0.9549476 * x * x * x - 1.37418593 * x * x + 2.09137015 * x - 0.16748867
    } else {
        3.0817580 * x * x * x - 5.87338670 * x * x + 3.75112997 * x - 0.37001483
    };
    (x, y)
}

/// CIE XYZ -> ProPhoto RGB, the space this pipeline actually works in. Equal
/// to `SRGB_TO_PROPHOTO · XYZ_TO_SRGB` (see lib.rs for the former), folded
/// into one matrix so a white point can be taken straight to pipe primaries.
const XYZ_TO_PROPHOTO: [[f32; 3]; 3] = [
    [1.3974796, -0.2141992, -0.1045309],
    [-0.5346504, 1.4943374, 0.0126436],
    [-0.0015093, -0.0041227, 0.9237237],
];

/// Per-channel gains for the temperature slider, in pipe (ProPhoto) primaries.
///
/// The old model was `R = 1 + 0.6t`, `B = 1 - 0.6t`, green untouched. Two
/// things were wrong with it. The Planckian locus is a curve, but that is a
/// straight line, so it pinned green to the exact midpoint of red and blue at
/// every temperature — whereas the real locus needs green slightly above the
/// midpoint when warming and well below it when cooling. And the constants
/// were sRGB-shaped while the multiply happens in ProPhoto, whose very wide
/// red primary needs almost no boost to warm an image: at 7188 K the correct
/// red gain is 1.043, not the 1.225 that model applied. That excess red on
/// top of a blue sky is what Francis saw as "beaucoup vers le magenta, pas le
/// chaud" (2026-09-22).
///
/// Now: convert both the base and target temperature to a white point on the
/// locus, take each to ProPhoto, and divide.
///
/// Returns the red and blue gains only — green is normalised to exactly 1, so
/// the control stays a pure chromaticity move and leaves brightness to
/// exposure. Green still *moves relative to* red and blue, which is the whole
/// correction; it just does so by them moving around it.
fn temperature_gains(slider: f32) -> (f32, f32) {
    if slider == 0.0 {
        return (1.0, 1.0);
    }
    let kelvin = WB_BASE_KELVIN
        + slider * if slider <= 0.0 { WB_COOL_PER_UNIT } else { WB_WARM_PER_UNIT };

    let white = |k: f32| {
        let (x, y) = planckian_xy(k);
        let xyz = [x / y, 1.0, (1.0 - x - y) / y];
        let mut rgb = [0.0f32; 3];
        for (i, row) in XYZ_TO_PROPHOTO.iter().enumerate() {
            rgb[i] = row[0] * xyz[0] + row[1] * xyz[1] + row[2] * xyz[2];
        }
        rgb
    };

    let base = white(WB_BASE_KELVIN);
    let target = white(kelvin);
    // Guard the divide: the fit can't return zero in the clamped range, but a
    // NaN here would poison every pixel rather than one.
    let gain = |i: usize| {
        if target[i].abs() < 1e-6 { 1.0 } else { (base[i] / target[i]).clamp(0.05, 20.0) }
    };
    let (r, g, b) = (gain(0), gain(1), gain(2));
    (r / g, b / g)
}

/// ProPhoto RGB (D50) -> Rec.709 / linear sRGB (D65). This is the exact
/// inverse of the pipeline's `SRGB_TO_PROPHOTO` (colour-science CAT02), so the
/// round-trip sRGB→ProPhoto→Rec709 is the identity. The previous matrix here
/// was non-physical (a `[0,0,1]` blue row) — white-preserving but hue-warping,
/// which is what tinted skin and desaturated colours in the Rapid render.
const PROPHOTO_TO_REC709: [[f32; 3]; 3] = [
    [2.0362741263, -0.7375868484, -0.2991716804],
    [-0.2256519640, 1.2230755666, 0.0027110556],
    [-0.0105510879, -0.1348857077, 1.1451776386],
];

/// Luminance weights for the pipeline's own space (linear ProPhoto RGB).
///
/// Equal to the Rec.709 weights times `PROPHOTO_TO_REC709` — i.e. exactly
/// "convert to Rec.709, then take its luma", precomputed. `luma_weights_
/// match_the_conversion_matrix` asserts that, so the two can't drift apart.
///
/// Every stage before the Rec.709 conversion used the Rec.709 weights
/// directly, which is the same mistake the temperature model made: constants
/// shaped for one space applied in another. It hides on neutrals — both sets
/// sum to 1, so a grey is identical either way — and only shows on saturated
/// colour, where ProPhoto's blue primary carries almost no luminance (0.021,
/// not 0.072). A saturated blue was being treated as 92% brighter than it is,
/// which put blue skies in the wrong tonal zone for every masked adjustment.
/// Normalised to sum to exactly 1 — the stored `PROPHOTO_TO_REC709` is a
/// rounded inverse, so the raw product sums to 0.999975 and a neutral would
/// drift by 2.5e-5 on every pass. Harmless in magnitude, but "a grey stays
/// itself" is an invariant worth holding exactly rather than nearly.
const PROPHOTO_LUMA: [f32; 3] = [0.2707707, 0.7082119, 0.0210174];

/// Luminance in the pipeline's working space. Use this anywhere before the
/// Rec.709 conversion.
#[inline]
fn luma(r: f32, g: f32, b: f32) -> f32 {
    PROPHOTO_LUMA[0] * r + PROPHOTO_LUMA[1] * g + PROPHOTO_LUMA[2] * b
}

/// Luminance for values already converted to Rec.709 — i.e. after
/// `PROPHOTO_TO_REC709`, which in practice means post-AgX display-referred
/// pixels. Separate from `luma` so the space is stated at the call site.
#[inline]
fn luma_709(r: f32, g: f32, b: f32) -> f32 {
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Shadow/midtone/highlight membership as a per-pixel luminance-weighted
/// crossfade — a linear 3-way partition (the three weights always sum to
/// exactly 1), not three independent gaussians. Shared by the 3-way color
/// wheels and Zone Tone Shaping so "what counts as a shadow" is defined
/// identically everywhere in this engine.
fn zone_weights(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let lum_linear = (luma(r, g, b)).max(0.0);
    let lum_norm = lum_linear.sqrt().min(1.0);
    let shadow_weight = (1.0 - lum_norm * 2.0).clamp(0.0, 1.0);
    let highlight_weight = ((lum_norm - 0.5) * 2.0).clamp(0.0, 1.0);
    let midtone_weight = (1.0 - shadow_weight - highlight_weight).max(0.0);
    (shadow_weight, midtone_weight, highlight_weight)
}

fn apply_filmic_exposure(color_in: [f32; 3], brightness_adj: f32) -> [f32; 3] {
    if brightness_adj == 0.0 {
        return color_in;
    }
    const RATIONAL_CURVE_MIX: f32 = 0.95;
    const MIDTONE_STRENGTH: f32 = 1.2;
    const TOP_ANCHOR: f32 = 1.06;
    let r = color_in[0];
    let g = color_in[1];
    let b = color_in[2];
    let original_luma = luma(r, g, b);
    if original_luma.abs() < 0.00001 {
        return color_in;
    }
    let direct_adj = brightness_adj * (1.0 - RATIONAL_CURVE_MIX);
    let rational_adj = brightness_adj * RATIONAL_CURVE_MIX;
    let scale = 2.0f32.powf(direct_adj);
    let k = 2.0f32.powf(-rational_adj * MIDTONE_STRENGTH);
    let luma_abs = original_luma.abs();
    let luma_floor = (luma_abs / TOP_ANCHOR).floor() * TOP_ANCHOR;
    let luma_norm = (luma_abs - luma_floor) / TOP_ANCHOR;
    let shaped_norm = luma_norm / (luma_norm + (1.0 - luma_norm) * k);
    let shaped_luma_abs = luma_floor + (shaped_norm * TOP_ANCHOR);
    let new_luma = original_luma.signum() * shaped_luma_abs * scale;
    let chroma_r = r - original_luma;
    let chroma_g = g - original_luma;
    let chroma_b = b - original_luma;
    let total_luma_scale = new_luma / original_luma;
    let luma_weight = (new_luma.clamp(0.0, 2.0)) * 0.5;
    let dynamic_exp = 0.95 - luma_weight * 0.3; // mix(0.95, 0.65, luma_weight)
    let base_chroma_scale = total_luma_scale.powf(dynamic_exp);
    let highlight_rolloff = 1.0 / (1.0 + (new_luma - 0.9).max(0.0) * 2.0);
    let chroma_scale = base_chroma_scale * highlight_rolloff;
    [
        new_luma + chroma_r * chroma_scale,
        new_luma + chroma_g * chroma_scale,
        new_luma + chroma_b * chroma_scale,
    ]
}

fn get_blurred_luma(x: usize, y: usize, blurred: &[f32], dw: usize, dh: usize) -> f32 {
    let fx = (x as f32) / 8.0 - 0.5;
    let fy = (y as f32) / 8.0 - 0.5;

    let x0 = fx.floor().clamp(0.0, (dw - 1) as f32) as usize;
    let x1 = (x0 + 1).min(dw - 1);
    let y0 = fy.floor().clamp(0.0, (dh - 1) as f32) as usize;
    let y1 = (y0 + 1).min(dh - 1);

    let tx = (fx - x0 as f32).clamp(0.0, 1.0);
    let ty = (fy - y0 as f32).clamp(0.0, 1.0);

    let c00 = blurred[y0 * dw + x0];
    let c10 = blurred[y0 * dw + x1];
    let c01 = blurred[y1 * dw + x0];
    let c11 = blurred[y1 * dw + x1];

    let top = c00 * (1.0 - tx) + c10 * tx;
    let bottom = c01 * (1.0 - tx) + c11 * tx;
    top * (1.0 - ty) + bottom * ty
}

/// Integer 2D hash → [0, 1), exact for any `u32` coordinate. Replaced an
/// earlier float `fract()`-based hash (and the gradient noise built on it):
/// that one lost fractional bits once the coordinate's integer part grew
/// past a few thousand, which showed up as a visible diagonal moiré/mesh
/// pattern in SilverGrain on full-resolution (4000px+) photos instead of
/// random per-pixel noise. Bit-mixing avalanche (SplitMix32-style).
fn hash_2d_u32(x: u32, y: u32) -> f32 {
    let mut h = x.wrapping_mul(0x9E3779B1) ^ y.wrapping_mul(0x85EBCA77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h as f32) / (u32::MAX as f32)
}

fn apply_vibrance(r: f32, g: f32, b: f32, sat_adj: f32, vib_adj: f32) -> (f32, f32, f32) {
    let luma = luma(r, g, b);
    let mut pr = r;
    let mut pg = g;
    let mut pb = b;

    if sat_adj != 0.0 {
        let f = 1.0 + sat_adj;
        pr = luma + (pr - luma) * f;
        pg = luma + (pg - luma) * f;
        pb = luma + (pb - luma) * f;
    }

    if vib_adj == 0.0 {
        return (pr, pg, pb);
    }

    let c_max = pr.max(pg).max(pb);
    let c_min = pr.min(pg).min(pb);
    let delta = c_max - c_min;
    if delta < 0.02 {
        return (pr, pg, pb);
    }

    let current_sat = delta / c_max.max(0.001);
    if vib_adj > 0.0 {
        let sat_mask = 1.0 - smoothstep(0.4, 0.9, current_sat);
        let (hue, _, _) = rgb_to_hsl(pr.max(0.0), pg.max(0.0), pb.max(0.0));
        let skin_center = 25.0f32;
        let hue_dist = (hue - skin_center).abs();
        let hue_dist = hue_dist.min(360.0 - hue_dist);
        let is_skin = smoothstep(35.0, 10.0, hue_dist);
        let skin_dampener = 1.0 * (1.0 - is_skin) + 0.6 * is_skin;
        let amount = vib_adj * sat_mask * skin_dampener * 3.0;
        let f = 1.0 + amount;
        (
            luma + (pr - luma) * f,
            luma + (pg - luma) * f,
            luma + (pb - luma) * f,
        )
    } else {
        let desat_mask = 1.0 - smoothstep(0.2, 0.8, current_sat);
        let amount = vib_adj * desat_mask;
        let f = 1.0 + amount;
        (
            luma + (pr - luma) * f,
            luma + (pg - luma) * f,
            luma + (pb - luma) * f,
        )
    }
}

/// `apply_local_contrast` with the tonal mask supplied by the caller instead
/// of computed internally — lets a caller target a mask shape other than
/// the shadow/highlight-protected midtone band below (e.g. a luminance
/// "zone" weight for the Zone Tone Shaping sliders, where the whole point is
/// to reach all the way into the highlights, which the internal smoothstep
/// mask would otherwise suppress).
fn apply_local_contrast_masked(
    r: f32,
    g: f32,
    b: f32,
    t_blurred: f32,
    amount: f32,
    mask: f32,
) -> (f32, f32, f32) {
    if amount == 0.0 || mask < 0.001 {
        return (r, g, b);
    }
    // Every caller's slider (zone_*_contrast at -50..50, clarity at
    // -40..60, structure at -30..50 — all percent-like, matching this
    // group's saturation sliders) hands `amount` in here raw, but both
    // branches below treat it as a small fraction: the positive branch
    // feeds it straight into a base-2 EXPONENT (2^(log_ratio * amount)),
    // and the negative branch uses it as a 0..1 blend weight. Fed a raw
    // value in the tens, a single slider step already blew past what
    // either branch was designed for — reported live as "way too much"
    // from just one step off zero on zone contrast. /100 brings the full
    // slider range down to the -0.5..0.5-ish span both formulas expect.
    let amount = amount * 0.01;

    if amount < 0.0 {
        let blur_amount = -amount * mask;
        let center_luma = (luma(r, g, b)).max(0.0001);
        let scale = t_blurred / center_luma;
        let br = r * scale;
        let bg = g * scale;
        let bb = b * scale;
        return (
            r * (1.0 - blur_amount) + br * blur_amount,
            g * (1.0 - blur_amount) + bg * blur_amount,
            b * (1.0 - blur_amount) + bb * blur_amount,
        );
    }

    let center_luma = (luma(r, g, b)).max(0.0);
    let safe_center_luma = center_luma.max(0.0001);
    let safe_blurred_luma = t_blurred.max(0.0001);
    let log_ratio = (safe_center_luma / safe_blurred_luma).log2();
    let contrast_factor = 2.0f32.powf(log_ratio * amount);
    let fr = r * contrast_factor;
    let fg = g * contrast_factor;
    let fb = b * contrast_factor;

    (
        r * (1.0 - mask) + fr * mask,
        g * (1.0 - mask) + fg * mask,
        b * (1.0 - mask) + fb * mask,
    )
}

fn apply_local_contrast(r: f32, g: f32, b: f32, t_blurred: f32, amount: f32) -> (f32, f32, f32) {
    if amount == 0.0 {
        return (r, g, b);
    }

    let mask = if amount < 0.0 {
        1.0
    } else {
        let center_luma = (luma(r, g, b)).max(0.0);
        let shadow_protection = smoothstep(0.0, 0.03, center_luma);
        let highlight_protection = 1.0 - smoothstep(0.9, 1.0, center_luma);
        shadow_protection * highlight_protection
    };

    apply_local_contrast_masked(r, g, b, t_blurred, amount, mask)
}

fn apply_dehaze(r: f32, g: f32, b: f32, t_blurred: f32, amount: f32) -> (f32, f32, f32) {
    if amount == 0.0 {
        return (r, g, b);
    }

    let atmospheric_light = [0.95f32, 0.97f32, 1.0f32];
    let pixel_dark = r.min(g).min(b);
    let regional_dark = t_blurred * 0.9;

    if amount > 0.0 {
        let pixel_luma = (luma(r, g, b)).max(0.0);
        let edge_diff = (pixel_luma.sqrt() - t_blurred.sqrt()).abs();
        let halo_protection = smoothstep(0.02, 0.15, edge_diff);
        let spatial_dark = regional_dark * (1.0 - halo_protection) + pixel_dark * halo_protection;
        let safe_dark = (spatial_dark - 0.02).max(0.0);
        let mapped_haze = safe_dark / (safe_dark + 0.2);
        let t = (1.0 - amount * mapped_haze * 0.85).max(0.15);

        let mut rec_r = (r - atmospheric_light[0]) / t + atmospheric_light[0];
        let mut rec_g = (g - atmospheric_light[1]) / t + atmospheric_light[1];
        let mut rec_b = (b - atmospheric_light[2]) / t + atmospheric_light[2];

        let rec_luma = (luma(rec_r, rec_g, rec_b)).max(0.0);
        let shadow_lift = smoothstep(0.1, 0.0, rec_luma) * (1.0 - t) * 0.15;
        rec_r += shadow_lift;
        rec_g += shadow_lift;
        rec_b += shadow_lift;

        let haze_removed = 1.0 - t;
        let sat_boost = haze_removed * 0.5;
        let final_luma = (luma(rec_r, rec_g, rec_b)).max(0.0);

        (
            (final_luma + (rec_r - final_luma) * (1.0 + sat_boost)).max(0.0),
            (final_luma + (rec_g - final_luma) * (1.0 + sat_boost)).max(0.0),
            (final_luma + (rec_b - final_luma) * (1.0 + sat_boost)).max(0.0),
        )
    } else {
        let safe_dark = (regional_dark - 0.02).max(0.0);
        let mapped_depth = safe_dark / (safe_dark + 0.2);
        let depth_factor = 0.4 * (1.0 - mapped_depth) + 1.0 * mapped_depth;
        let f = amount.abs() * 0.7 * depth_factor;
        (
            r * (1.0 - f) + atmospheric_light[0] * f,
            g * (1.0 - f) + atmospheric_light[1] * f,
            b * (1.0 - f) + atmospheric_light[2] * f,
        )
    }
}

/// Process an `ImageBuf` using the Rapid digital RAW engine pipeline.
/// Pre-LUTs run on scene-linear input before exposure/tone controls; post-LUTs
/// run after tone mapping and grain on display-encoded output.
pub fn develop_rapid(input: &ImageBuf, recipe: &Recipe, luts_dir: &Path) -> ImageBuf {
    develop_rapid_with(input, recipe, luts_dir, crate::rapid_gpu::enabled())
}

/// `develop_rapid` with the GPU decision made by the caller. Exists so the
/// equivalence test can run the same frame down both paths in one process —
/// the CPU loop only counts as a fallback for as long as it agrees with the
/// shader, and that has to be checked, not assumed.
pub(crate) fn develop_rapid_with(
    input: &ImageBuf,
    recipe: &Recipe,
    luts_dir: &Path,
    use_gpu: bool,
) -> ImageBuf {
    let width = input.width as usize;
    let height = input.height as usize;
    let total_pixels = width * height;

    let mut work_input = (*input).clone();
    let pre_luts = load_lut_stack(&recipe.rapid_pre_luts, luts_dir);
    if !pre_luts.is_empty() {
        crate::lut::apply_stack_linear(&mut work_input, &pre_luts);
    }

    let exposure_factor = 2.0f32.powf(recipe.exposure_ev);
    let contrast = recipe.contrast;
    let saturation = (recipe.saturation + 1.0).max(0.0);

    let (r_temp, b_temp) = temperature_gains(recipe.temperature);

    // Tint (-100 to 100): the off-locus green/magenta axis. Unlike
    // temperature this genuinely is a simple push — "tint" is by definition
    // the deviation perpendicular to the Planckian curve.
    let tint_shift = recipe.tint / 100.0;
    let g_tint = (1.0 - tint_shift * 0.5).max(0.1);
    let r_tint = (1.0 + tint_shift * 0.25).max(0.1);
    let b_tint = (1.0 + tint_shift * 0.25).max(0.1);

    // Tonal controls
    let whites = recipe.whites / 100.0;
    let highlights = recipe.highlights / 100.0;
    let midtones = recipe.midtones / 100.0;
    let shadows = recipe.shadows / 100.0;

    let brightness = recipe.brightness / 100.0;
    let blacks = recipe.blacks / 100.0;
    let vibrance = recipe.vibrance / 100.0;
    let clarity = recipe.clarity / 100.0;
    let structure = recipe.structure / 100.0;
    let dehaze = recipe.dehaze / 100.0;

    let hsl_hues = &recipe.hsl_hue;
    let hsl_sats = &recipe.hsl_sat;
    let hsl_lums = &recipe.hsl_lum;
    // Any nonzero band means the user touched the HSL matrix — the recipe
    // default now ships a zeroed 8-length vec (so per-band sliders can bind
    // to it), which would otherwise make `len() >= 8` true even when unused.
    let has_hsl = hsl_hues.iter().any(|v| *v != 0.0)
        || hsl_sats.iter().any(|v| *v != 0.0)
        || hsl_lums.iter().any(|v| *v != 0.0);

    // Tone-curve LUTs are built once per render, not per pixel — an
    // identity curve (the default) builds nothing at all, so a recipe that
    // never touched a curve pays a single Option check per pixel.
    let build = |pts: &Vec<[f32; 2]>| {
        if crate::curves::is_identity(pts) {
            None
        } else {
            crate::curves::build_lut(pts)
        }
    };
    let curve_luma = build(&recipe.curve_luma);
    let curve_r = build(&recipe.curve_r);
    let curve_g = build(&recipe.curve_g);
    let curve_b = build(&recipe.curve_b);
    let has_curves =
        curve_luma.is_some() || curve_r.is_some() || curve_g.is_some() || curve_b.is_some();

    let shadows_tint = recipe.shadows_tint;
    let midtones_tint = recipe.midtones_tint;
    let highlights_tint = recipe.highlights_tint;
    let has_color_wheels = shadows_tint != [0.0, 0.0, 0.0]
        || midtones_tint != [0.0, 0.0, 0.0]
        || highlights_tint != [0.0, 0.0, 0.0];

    let zone_shadows_exposure = recipe.zone_shadows_exposure;
    let zone_shadows_contrast = recipe.zone_shadows_contrast;
    let zone_shadows_saturation = recipe.zone_shadows_saturation;
    let zone_midtones_exposure = recipe.zone_midtones_exposure;
    let zone_midtones_contrast = recipe.zone_midtones_contrast;
    let zone_midtones_saturation = recipe.zone_midtones_saturation;
    let zone_highlights_exposure = recipe.zone_highlights_exposure;
    let zone_highlights_contrast = recipe.zone_highlights_contrast;
    let zone_highlights_saturation = recipe.zone_highlights_saturation;
    let has_zones = zone_shadows_exposure != 0.0
        || zone_shadows_contrast != 0.0
        || zone_shadows_saturation != 0.0
        || zone_midtones_exposure != 0.0
        || zone_midtones_contrast != 0.0
        || zone_midtones_saturation != 0.0
        || zone_highlights_exposure != 0.0
        || zone_highlights_contrast != 0.0
        || zone_highlights_saturation != 0.0;
    let has_zone_contrast =
        zone_shadows_contrast != 0.0 || zone_midtones_contrast != 0.0 || zone_highlights_contrast != 0.0;

    // Global Whites multiplier (Whites processed first)
    let w_mult = if whites != 0.0 {
        let white_level = 1.0 - whites * 0.25;
        1.0 / white_level.max(0.01)
    } else {
        1.0
    };

    // Build blurred guidance map if shadows, blacks, clarity, structure,
    // dehaze, or a zone-contrast slider are active (Phase 2)
    let (blurred, down_w, down_h) = if shadows != 0.0
        || blacks != 0.0
        || clarity != 0.0
        || structure != 0.0
        || dehaze != 0.0
        || has_zone_contrast
        {
            let down_w = (width / 8).max(1);
            let down_h = (height / 8).max(1);
            let mut downsampled = vec![0.0f32; down_w * down_h];

            let brightness_adj = midtones + brightness;

            for dy in 0..down_h {
                for dx in 0..down_w {
                    let start_y = dy * 8;
                    let end_y = ((dy + 1) * 8).min(height);
                    let start_x = dx * 8;
                    let end_x = ((dx + 1) * 8).min(width);
                    let count = ((end_y - start_y) * (end_x - start_x)) as f32;

                    let mut r_sum = 0.0f32;
                    let mut g_sum = 0.0f32;
                    let mut b_sum = 0.0f32;
                    for y in start_y..end_y {
                        for x in start_x..end_x {
                            let idx = (y * width + x) * 3;
                            r_sum += work_input.data[idx];
                            g_sum += work_input.data[idx + 1];
                            b_sum += work_input.data[idx + 2];
                        }
                    }
                    let r_avg = (r_sum / count) * w_mult * exposure_factor * r_temp * r_tint;
                    let g_avg = (g_sum / count) * w_mult * exposure_factor * g_tint;
                    let b_avg = (b_sum / count) * w_mult * exposure_factor * b_temp * b_tint;

                    // Filmic Exposure (using recipe.midtones + recipe.brightness)
                    let [r_proc, g_proc, b_proc] = if brightness_adj != 0.0 {
                        apply_filmic_exposure([r_avg, g_avg, b_avg], brightness_adj)
                    } else {
                        [r_avg, g_avg, b_avg]
                    };

                    let luma_linear =
                        (luma(r_proc, g_proc, b_proc)).max(0.0);
                    downsampled[dy * down_w + dx] = luma_linear.max(0.0001).powf(0.4545);
                }
            }

            // Fast 2-pass box blur (radius 2, box size 5)
            let r_blur = 2;
            let mut temp = vec![0.0f32; down_w * down_h];
            for y in 0..down_h {
                for x in 0..down_w {
                    let mut sum = 0.0f32;
                    let mut count = 0.0f32;
                    for dx in -(r_blur as i32)..=(r_blur as i32) {
                        let nx = (x as i32 + dx).clamp(0, down_w as i32 - 1) as usize;
                        sum += downsampled[y * down_w + nx];
                        count += 1.0;
                    }
                    temp[y * down_w + x] = sum / count;
                }
            }

            let mut blurred_buf = vec![0.0f32; down_w * down_h];
            for x in 0..down_w {
                for y in 0..down_h {
                    let mut sum = 0.0f32;
                    let mut count = 0.0f32;
                    for dy in -(r_blur as i32)..=(r_blur as i32) {
                        let ny = (y as i32 + dy).clamp(0, down_h as i32 - 1) as usize;
                        sum += temp[ny * down_w + x];
                        count += 1.0;
                    }
                    blurred_buf[y * down_w + x] = sum / count;
                }
            }

            (blurred_buf, down_w, down_h)
        } else {
            (Vec::new(), 0, 0)
        };

    // The per-pixel stage on the GPU when there is one. Everything above
    // (guidance map) and below (grain, post-LUTs) stays on the CPU. A `None`
    // here means the GPU couldn't, never that it produced nothing — the
    // Rayon loop below is both the reference implementation and the
    // fallback, so a photo always develops.
    let gpu_out = if use_gpu {
        crate::rapid_gpu::run(
        &crate::rapid_gpu::Inputs {
            width,
            height,
            data: &work_input.data,
            blurred: &blurred,
            down_w,
            down_h,
            w_mult,
            exposure_factor,
            r_temp,
            r_tint,
            g_tint,
            b_temp,
            b_tint,
            brightness_adj: midtones + brightness,
            saturation_adj: saturation - 1.0,
            contrast,
            shadows,
            blacks,
            highlights,
            clarity,
            structure,
            dehaze,
            vibrance,
            has_hsl,
            has_color_wheels,
            has_zones,
            curves: [
                curve_luma.clone(),
                curve_r.clone(),
                curve_g.clone(),
                curve_b.clone(),
            ],
        },
        recipe,
        )
    } else {
        None
    };

    let used_gpu = gpu_out.is_some();
    let mut out_data = match gpu_out {
        Some(img) => img.data,
        None => vec![0.0f32; total_pixels * 3],
    };

    // Process pixels in parallel chunks using Rayon
    if !used_gpu {
    out_data
        .par_chunks_exact_mut(3)
        .enumerate()
        .for_each(|(idx, pixel)| {
            let in_idx = idx * 3;
            let x_coord = idx % width;
            let y_coord = idx / width;

            // 1. Whites multiplier, Exposure, Temp & Tint WB
            let mut r = work_input.data[in_idx] * w_mult * exposure_factor * r_temp * r_tint;
            let mut g = work_input.data[in_idx + 1] * w_mult * exposure_factor * g_tint;
            let mut b = work_input.data[in_idx + 2] * w_mult * exposure_factor * b_temp * b_tint;

            // Clarity, Structure, Dehaze (using guidance map)
            if (clarity != 0.0 || structure != 0.0 || dehaze != 0.0) && !blurred.is_empty() {
                let t_blurred = get_blurred_luma(x_coord, y_coord, &blurred, down_w, down_h);

                if clarity != 0.0 {
                    let (cr, cg, cb) = apply_local_contrast(r, g, b, t_blurred, clarity);
                    r = cr;
                    g = cg;
                    b = cb;
                }
                if structure != 0.0 {
                    let (sr, sg, sb) = apply_local_contrast(r, g, b, t_blurred, structure);
                    r = sr;
                    g = sg;
                    b = sb;
                }
                if dehaze != 0.0 {
                    let (dr, dg, db) = apply_dehaze(r, g, b, t_blurred, dehaze);
                    r = dr;
                    g = dg;
                    b = db;
                }
            }

            // 2. Filmic Exposure / Brightness (using recipe.midtones + recipe.brightness)
            let brightness_adj = midtones + brightness;
            if brightness_adj != 0.0 {
                let color_bright = apply_filmic_exposure([r, g, b], brightness_adj);
                r = color_bright[0];
                g = color_bright[1];
                b = color_bright[2];
            }

            // 3. Contrast: perceptual S-curve contrast around 0.5 in 1/2.2 space
            if contrast.abs() > 1e-4 {
                let g_power = 2.2f32;
                let strength = 2.0f32.powf(contrast * 1.25);

                let apply_contrast = |val: f32| -> f32 {
                    let safe_val = val.max(0.0);
                    let perceptual = safe_val.powf(1.0 / g_power).min(1.0);
                    let curved = if perceptual < 0.5 {
                        0.5 * (2.0 * perceptual).powf(strength)
                    } else {
                        1.0 - 0.5 * (2.0 * (1.0 - perceptual)).powf(strength)
                    };
                    let contrast_adjusted = curved.powf(g_power);
                    let mix_factor = smoothstep(1.0, 1.01, safe_val);
                    contrast_adjusted * (1.0 - mix_factor) + safe_val * mix_factor
                };

                r = apply_contrast(r);
                g = apply_contrast(g);
                b = apply_contrast(b);
            }

            // 4. Shadows & Blacks (per-pixel pivot-contrasted lift with detail recovery)
            if shadows != 0.0 || blacks != 0.0 {
                let luma_linear = (luma(r, g, b)).max(0.0);
                let safe_pixel_luma = luma_linear.max(0.0001);
                let t_pixel = safe_pixel_luma.powf(0.4545);

                // Lookup blurred guidance luma
                let t_blurred = get_blurred_luma(x_coord, y_coord, &blurred, down_w, down_h);

                // Shadow lift profile: sh * t * (1-t)^4.5
                let shadow_lift = shadows * t_pixel * (1.0 - t_pixel).max(0.0).powf(4.5);
                // Black lift profile: bl * t * (1-t)^12.0
                let black_lift = blacks * t_pixel * (1.0 - t_pixel).max(0.0).powf(12.0);
                let lift_amount = (shadow_lift + black_lift).max(0.0);

                let t_pixel_curved = (t_pixel + shadow_lift + black_lift).max(0.0);

                // Stretch pivot contrast around 0.2
                let shadow_pivot = 0.2f32;
                let stretch_factor = 1.0 + (lift_amount * 1.3);
                let contrasted_t = shadow_pivot + (t_pixel_curved - shadow_pivot) * stretch_factor;

                let final_t = (t_pixel_curved * 0.15 + contrasted_t * 0.85).max(0.0);
                let curved_luma = final_t.powf(2.2);

                let luma_ratio = curved_luma / safe_pixel_luma;
                r *= luma_ratio;
                g *= luma_ratio;
                b *= luma_ratio;

                // Detail preservation / local tone mapping
                let detail = t_pixel / t_blurred.max(0.0001);
                let safe_detail = detail.clamp(0.8, 1.25);
                let noise_protection = smoothstep(0.0, 0.1, t_blurred);
                let detail_amp = 1.0 + lift_amount * 1.2 * noise_protection;
                let enhanced_detail = safe_detail.powf(detail_amp);
                let detail_correction = enhanced_detail / safe_detail;
                let linear_correction = detail_correction.powf(2.2);

                r *= linear_correction;
                g *= linear_correction;
                b *= linear_correction;

                let final_luma_ratio = luma_ratio * linear_correction;
                if final_luma_ratio > 1.0 {
                    let recovered_luma = luma(r, g, b);
                    let boost_amount = ((final_luma_ratio - 1.0) * 0.15).clamp(0.0, 0.4);
                    r = r * (1.0 - boost_amount) + recovered_luma * boost_amount;
                    g = g * (1.0 - boost_amount) + recovered_luma * boost_amount;
                    b = b * (1.0 - boost_amount) + recovered_luma * boost_amount;
                }
            }

            // 5. Highlights (rational compression + white desaturation)
            if highlights != 0.0 {
                let pixel_luma = (luma(r, g, b)).max(0.0);
                let safe_pixel_luma = pixel_luma.max(0.0001);

                let pixel_mask_input = (safe_pixel_luma * 1.5).tanh();
                let highlight_mask = smoothstep(0.3, 0.95, pixel_mask_input);

                if highlight_mask > 0.001 {
                    let (final_adjusted_r, final_adjusted_g, final_adjusted_b) = if highlights < 0.0
                    {
                        let new_luma = if pixel_luma <= 1.0 {
                            pixel_luma.powf(1.0 - highlights * 1.75)
                        } else {
                            let luma_excess = pixel_luma - 1.0;
                            let compression_strength = -highlights * 6.0;
                            let compressed_excess =
                                luma_excess / (1.0 + luma_excess * compression_strength);
                            1.0 + compressed_excess
                        };

                        let luma_scale = new_luma / safe_pixel_luma;
                        let tonally_adjusted_r = r * luma_scale;
                        let tonally_adjusted_g = g * luma_scale;
                        let tonally_adjusted_b = b * luma_scale;

                        let desaturation_amount = smoothstep(1.0, 10.0, pixel_luma);

                        (
                            tonally_adjusted_r * (1.0 - desaturation_amount)
                                + new_luma * desaturation_amount,
                            tonally_adjusted_g * (1.0 - desaturation_amount)
                                + new_luma * desaturation_amount,
                            tonally_adjusted_b * (1.0 - desaturation_amount)
                                + new_luma * desaturation_amount,
                        )
                    } else {
                        let adjustment = highlights * 1.75;
                        let factor = 2.0f32.powf(adjustment);
                        (r * factor, g * factor, b * factor)
                    };

                    r = r * (1.0 - highlight_mask) + final_adjusted_r * highlight_mask;
                    g = g * (1.0 - highlight_mask) + final_adjusted_g * highlight_mask;
                    b = b * (1.0 - highlight_mask) + final_adjusted_b * highlight_mask;
                }
            }

            // 6. 3-Way Color Wheels
            if has_color_wheels {
                let (shadow_weight, midtone_weight, highlight_weight) = zone_weights(r, g, b);

                r += shadows_tint[0] * shadow_weight * 0.2
                    + midtones_tint[0] * midtone_weight * 0.2
                    + highlights_tint[0] * highlight_weight * 0.2;
                g += shadows_tint[1] * shadow_weight * 0.2
                    + midtones_tint[1] * midtone_weight * 0.2
                    + highlights_tint[1] * highlight_weight * 0.2;
                b += shadows_tint[2] * shadow_weight * 0.2
                    + midtones_tint[2] * midtone_weight * 0.2
                    + highlights_tint[2] * highlight_weight * 0.2;
            }

            // 6a. Zone Tone Shaping — mask-free Shadows/Midtones/Highlights
            // local exposure/contrast/saturation, approximating what
            // Lightroom's luminosity-range local masks do via the same
            // luminance-weighted soft zones as the color wheels above,
            // reusing the same `blurred` guidance map clarity/structure use.
            if has_zones {
                let (shadow_weight, midtone_weight, highlight_weight) = zone_weights(r, g, b);

                let blended_ev = zone_shadows_exposure * shadow_weight
                    + zone_midtones_exposure * midtone_weight
                    + zone_highlights_exposure * highlight_weight;
                if blended_ev != 0.0 {
                    let factor = 2f32.powf(blended_ev);
                    r *= factor;
                    g *= factor;
                    b *= factor;
                }

                if !blurred.is_empty() {
                    let t_blurred = get_blurred_luma(x_coord, y_coord, &blurred, down_w, down_h);
                    if zone_shadows_contrast != 0.0 {
                        let (nr, ng, nb) = apply_local_contrast_masked(
                            r,
                            g,
                            b,
                            t_blurred,
                            zone_shadows_contrast,
                            shadow_weight,
                        );
                        r = nr;
                        g = ng;
                        b = nb;
                    }
                    if zone_midtones_contrast != 0.0 {
                        let (nr, ng, nb) = apply_local_contrast_masked(
                            r,
                            g,
                            b,
                            t_blurred,
                            zone_midtones_contrast,
                            midtone_weight,
                        );
                        r = nr;
                        g = ng;
                        b = nb;
                    }
                    if zone_highlights_contrast != 0.0 {
                        let (nr, ng, nb) = apply_local_contrast_masked(
                            r,
                            g,
                            b,
                            t_blurred,
                            zone_highlights_contrast,
                            highlight_weight,
                        );
                        r = nr;
                        g = ng;
                        b = nb;
                    }
                }

                let blended_sat = zone_shadows_saturation * shadow_weight
                    + zone_midtones_saturation * midtone_weight
                    + zone_highlights_saturation * highlight_weight;
                if blended_sat != 0.0 {
                    let luma = luma(r, g, b);
                    let sat_factor = (1.0 + blended_sat / 100.0).max(0.0);
                    r = luma + (r - luma) * sat_factor;
                    g = luma + (g - luma) * sat_factor;
                    b = luma + (b - luma) * sat_factor;
                }
            }

            // 7. Saturation, Vibrance & HSL Matrix
            if (saturation - 1.0).abs() > 1e-4 || vibrance != 0.0 || has_hsl {
                let (nr, ng, nb) = apply_vibrance(r, g, b, saturation - 1.0, vibrance);
                if has_hsl {
                    let (h, mut s, mut l) = rgb_to_hsl(nr.max(0.0), ng.max(0.0), nb.max(0.0));
                    let mut hue_adj = 0.0f32;
                    let mut sat_adj = 0.0f32;
                    let mut lum_adj = 0.0f32;

                    for i in 0..8 {
                        let center = HUE_CENTERS[i];
                        let dist = hue_distance(h, center);
                        if dist < 45.0 {
                            let weight = (1.0 - dist / 45.0).max(0.0);
                            hue_adj += hsl_hues.get(i).copied().unwrap_or(0.0) * weight;
                            sat_adj += hsl_sats.get(i).copied().unwrap_or(0.0) * weight;
                            lum_adj += hsl_lums.get(i).copied().unwrap_or(0.0) * weight;
                        }
                    }

                    let new_h = (h + hue_adj).rem_euclid(360.0);
                    s = (s * (1.0 + sat_adj / 100.0)).clamp(0.0, 1.0);
                    l = (l * (1.0 + lum_adj / 100.0)).max(0.0);

                    let (nnr, nng, nnb) = hsl_to_rgb(new_h, s, l);
                    r = nnr;
                    g = nng;
                    b = nnb;
                } else {
                    r = nr;
                    g = ng;
                    b = nb;
                }
            }

            // 8. Vignetting
            if recipe.vignette_amount != 0.0 {
                let w_f = width as f32;
                let h_f = height as f32;
                let aspect = h_f / w_f;
                let x_f = x_coord as f32;
                let y_f = y_coord as f32;
                let uv_x = (x_f / w_f - 0.5) * 2.0;
                let uv_y = (y_f / h_f - 0.5) * 2.0;

                let v_round = 1.0 - recipe.vignette_roundness;
                let v_feather = recipe.vignette_feather * 0.5;
                let v_mid = recipe.vignette_midpoint;

                let uv_round_x = uv_x.signum() * uv_x.abs().powf(v_round);
                let uv_round_y = uv_y.signum() * uv_y.abs().powf(v_round);

                let dist = (uv_round_x * uv_round_x + uv_round_y * uv_round_y * aspect * aspect)
                    .sqrt()
                    * 0.5;
                let vignette_mask = smoothstep(v_mid - v_feather, v_mid + v_feather, dist);

                if recipe.vignette_amount < 0.0 {
                    let factor = 1.0 + recipe.vignette_amount * vignette_mask;
                    r *= factor;
                    g *= factor;
                    b *= factor;
                } else {
                    let amount = recipe.vignette_amount * vignette_mask;
                    r = r * (1.0 - amount) + amount;
                    g = g * (1.0 - amount) + amount;
                    b = b * (1.0 - amount) + amount;
                }
            }

            // Convert ProPhoto RGB (D50) -> Rec.709 Linear (D65) for AgX
            let r_709 = (PROPHOTO_TO_REC709[0][0] * r
                + PROPHOTO_TO_REC709[0][1] * g
                + PROPHOTO_TO_REC709[0][2] * b)
                .max(0.0);
            let g_709 = (PROPHOTO_TO_REC709[1][0] * r
                + PROPHOTO_TO_REC709[1][1] * g
                + PROPHOTO_TO_REC709[1][2] * b)
                .max(0.0);
            let b_709 = (PROPHOTO_TO_REC709[2][0] * r
                + PROPHOTO_TO_REC709[2][1] * g
                + PROPHOTO_TO_REC709[2][2] * b)
                .max(0.0);

            if recipe.use_logc {
                // Emit raw ARRI LogC3 — intended to feed a LogC-authored print LUT
                // (e.g. Brim 2383). LogC is *supposed* to look flat/dark on its own;
                // the print LUT supplies the display rendering. The previous
                // srgb_encode(logc3_encode(...)) wrap double-mapped the signal and
                // made logc + print-LUT render "too bright".
                pixel[0] = logc3_encode(r_709.max(0.0));
                pixel[1] = logc3_encode(g_709.max(0.0));
                pixel[2] = logc3_encode(b_709.max(0.0));
            } else {
                let (mut agx_r, mut agx_g, mut agx_b) = agx_tonemap(r_709, g_709, b_709);

                agx_r = agx_r.max(0.0);
                agx_g = agx_g.max(0.0);
                agx_b = agx_b.max(0.0);

                match recipe.agx_look.as_str() {
                    "punchy" => {
                        agx_r = agx_r.powf(1.15);
                        agx_g = agx_g.powf(1.15);
                        agx_b = agx_b.powf(1.15);
                    }
                    "golden" => {
                        agx_r = (agx_r * 1.04).min(1.0);
                        agx_g = (agx_g * 1.01).min(1.0);
                        agx_b = (agx_b * 0.95).max(0.0);
                    }
                    "soft" => {
                        agx_r = agx_r.powf(0.88);
                        agx_g = agx_g.powf(0.88);
                        agx_b = agx_b.powf(0.88);
                    }
                    "bw" => {
                        let luma = luma_709(agx_r, agx_g, agx_b);
                        agx_r = luma;
                        agx_g = luma;
                        agx_b = luma;
                    }
                    _ => {}
                }

                let max_c = agx_r.max(agx_g).max(agx_b);
                if max_c > 0.85 && recipe.highlight_desat > 0.0 {
                    let desat_w =
                        ((max_c - 0.85) / 0.15).clamp(0.0, 1.0) * recipe.highlight_desat;
                    let avg_c = (agx_r + agx_g + agx_b) / 3.0;
                    agx_r = agx_r * (1.0 - desat_w) + avg_c * desat_w;
                    agx_g = agx_g * (1.0 - desat_w) + avg_c * desat_w;
                    agx_b = agx_b * (1.0 - desat_w) + avg_c * desat_w;
                }

                // Tone curves last, on display-referred 0..1 values — that's
                // the space a point curve is drawn in and reasoned about
                // (the histogram under the editor is this same space).
                // Applying them before the tone map would make the curve's
                // own shape meaningless, since AgX would reshape it again.
                if has_curves {
                    agx_r = agx_r.clamp(0.0, 1.0);
                    agx_g = agx_g.clamp(0.0, 1.0);
                    agx_b = agx_b.clamp(0.0, 1.0);
                    if let Some(lut) = &curve_luma {
                        agx_r = crate::curves::sample(lut, agx_r);
                        agx_g = crate::curves::sample(lut, agx_g);
                        agx_b = crate::curves::sample(lut, agx_b);
                    }
                    if let Some(lut) = &curve_r {
                        agx_r = crate::curves::sample(lut, agx_r);
                    }
                    if let Some(lut) = &curve_g {
                        agx_g = crate::curves::sample(lut, agx_g);
                    }
                    if let Some(lut) = &curve_b {
                        agx_b = crate::curves::sample(lut, agx_b);
                    }
                }

                pixel[0] = agx_r.clamp(0.0, 1.0);
                pixel[1] = agx_g.clamp(0.0, 1.0);
                pixel[2] = agx_b.clamp(0.0, 1.0);
            }
        });
    } // !used_gpu — the CPU loop is both the reference and the fallback

    if recipe.grain_amount > 1e-4 {
        apply_silvergrain(
            &mut out_data,
            width as u32,
            height as u32,
            recipe.grain_amount,
            recipe.grain_roughness,
        );
    }

    let post_luts = load_lut_stack(&recipe.rapid_post_luts, luts_dir);
    let mut result = ImageBuf::from_data(width as u32, height as u32, out_data);
    if !post_luts.is_empty() {
        crate::lut::apply_stack_display(&mut result, &post_luts);
    }

    result
}

/// AgX Inset Matrix (linear Rec.709/sRGB pipe → AgX rendering primaries).
/// Derived the same way as RapidRAW's `calculate_agx_matrices_glam`:
/// sRGB-pipe → Rec.2020 base → rotated/scaled AgX rendering primaries
/// (D65 throughout). The previous constants (0.84247…/1.19687…) were the old
/// Blender AgX *mini* matrices, which assume a different working space and
/// produced the dark, desaturated render.
const AGX_INSET: [[f32; 3]; 3] = [
    [0.5682423421, 0.3731251307, 0.05863252723],
    [0.1281182356, 0.7783136252, 0.09356813916],
    [0.07347080765, 0.1620963122, 0.7644328802],
];

/// AgX Outset Matrix (AgX rendering primaries → linear Rec.709/sRGB pipe).
/// Inverse-paired with AGX_INSET above.
const AGX_OUTSET: [[f32; 3]; 3] = [
    [1.940429221, -0.8296109087, -0.110818312],
    [-0.276003293, 1.30673334, -0.03073004751],
    [-0.1438899598, -0.2248403189, 1.368730279],
];

/// AgX constants — verbatim from RapidRAW (`shader.wgsl` /
/// `apply_cpu_agx_tonemap`). The previous code used a Hermite smoothstep and
/// a −10/+6.5 EV window with no output gamma; that crushed the midtones and
/// clipped deep shadows, which is why every Rapid render read ~1 stop dark.
const AGX_EPSILON: f32 = 1.0e-6;
const AGX_MIN_EV: f32 = -15.2;
const AGX_MAX_EV: f32 = 5.0;
const AGX_RANGE_EV: f32 = AGX_MAX_EV - AGX_MIN_EV;
const AGX_GAMMA: f32 = 2.4; // output EOTF — lifts the curve into display range
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
/// 18% middle-grey — the reference divides by this before log2 so the EV
/// window centres on grey. Without it the whole curve shifts one stop.
const AGX_MIDDLE_GREY: f32 = 0.18;

/// AgX Filmic Tone Mapper — faithful port of RapidRAW's `agx_tonemap`
/// (`shader.wgsl:1155` / `apply_cpu_agx_tonemap`). Compresses HDR linear
/// values into display range with a logistic toe/shoulder and a linear
/// midsection, then applies the `^2.4` output gamma. Input is linear
/// Rec.709/sRGB (our `PROPHOTO_TO_REC709` lands us there).
fn agx_tonemap(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    // 0. Gamut compression — push negative values (from the inset matrix or
    //    out-of-gamut saturated colours) up to zero so log2 can't see a
    //    negative and produce NaN. Matches `agx_compress_gamut`.
    let min_c = r.min(g).min(b);
    let (r, g, b) = if min_c < 0.0 {
        (r - min_c, g - min_c, b - min_c)
    } else {
        (r, g, b)
    };

    // 1. Matrix Inset (pipe → AgX rendering primaries)
    let r_in = AGX_INSET[0][0] * r + AGX_INSET[0][1] * g + AGX_INSET[0][2] * b;
    let g_in = AGX_INSET[1][0] * r + AGX_INSET[1][1] * g + AGX_INSET[1][2] * b;
    let b_in = AGX_INSET[2][0] * r + AGX_INSET[2][1] * g + AGX_INSET[2][2] * b;

    // 2. Normalize to 18% grey, then log2-encode over the −15.2..+5.0 EV span.
    let log_r = ((r_in / AGX_MIDDLE_GREY).max(AGX_EPSILON).log2() - AGX_MIN_EV) / AGX_RANGE_EV;
    let log_g = ((g_in / AGX_MIDDLE_GREY).max(AGX_EPSILON).log2() - AGX_MIN_EV) / AGX_RANGE_EV;
    let log_b = ((b_in / AGX_MIDDLE_GREY).max(AGX_EPSILON).log2() - AGX_MIN_EV) / AGX_RANGE_EV;

    let mapped_r = log_r.clamp(0.0, 1.0);
    let mapped_g = log_g.clamp(0.0, 1.0);
    let mapped_b = log_b.clamp(0.0, 1.0);

    // 3. Piecewise contrast curve (logistic toe + linear mid + logistic shoulder)
    let curved_r = agx_curve_channel(mapped_r);
    let curved_g = agx_curve_channel(mapped_g);
    let curved_b = agx_curve_channel(mapped_b);

    // 4. Output gamma (the EOTF the old code was missing — lifts midtones).
    let gr = curved_r.max(0.0).powf(AGX_GAMMA);
    let gg = curved_g.max(0.0).powf(AGX_GAMMA);
    let gb = curved_b.max(0.0).powf(AGX_GAMMA);

    // 5. Matrix Outset (AgX rendering primaries → pipe)
    let r_out = AGX_OUTSET[0][0] * gr + AGX_OUTSET[0][1] * gg + AGX_OUTSET[0][2] * gb;
    let g_out = AGX_OUTSET[1][0] * gr + AGX_OUTSET[1][1] * gg + AGX_OUTSET[1][2] * gb;
    let b_out = AGX_OUTSET[2][0] * gr + AGX_OUTSET[2][1] * gg + AGX_OUTSET[2][2] * gb;

    (r_out, g_out, b_out)
}

/// Generalized logistic: `x / (1 + x^p)^(1/p)`. Verbatim from the reference's
/// `agx_sigmoid` — not a Hermite smoothstep.
fn agx_sigmoid(x: f32, power: f32) -> f32 {
    x / (1.0 + x.powf(power)).powf(1.0 / power)
}

/// Scaled + translated sigmoid used for the toe and shoulder segments.
fn agx_scaled_sigmoid(x: f32, scale: f32, slope: f32, power: f32, tx: f32, ty: f32) -> f32 {
    scale * agx_sigmoid(slope * (x - tx) / scale, power) + ty
}

/// AgX piecewise contrast curve: logistic toe below the transition, a linear
/// segment through the midtones, and a logistic shoulder above. This is what
/// gives AgX its filmic rolloff; the old smoothstep flattened it.
fn agx_curve_channel(x: f32) -> f32 {
    let result = if x < AGX_TOE_TRANSITION_X {
        agx_scaled_sigmoid(
            x,
            AGX_TOE_SCALE,
            AGX_SLOPE,
            AGX_TOE_POWER,
            AGX_TOE_TRANSITION_X,
            AGX_TOE_TRANSITION_Y,
        )
    } else if x <= AGX_SHOULDER_TRANSITION_X {
        AGX_SLOPE * x + AGX_INTERCEPT
    } else {
        agx_scaled_sigmoid(
            x,
            AGX_SHOULDER_SCALE,
            AGX_SLOPE,
            AGX_SHOULDER_POWER,
            AGX_SHOULDER_TRANSITION_X,
            AGX_SHOULDER_TRANSITION_Y,
        )
    };
    result.clamp(0.0, 1.0)
}

fn hue_distance(h1: f32, h2: f32) -> f32 {
    let d = (h1 - h2).abs() % 360.0;
    if d > 180.0 {
        360.0 - d
    } else {
        d
    }
}

fn rgb_to_hsl(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let l = (max + min) / 2.0;

    if delta.abs() < 1e-6 {
        return (0.0, 0.0, l);
    }

    let s = if l > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };

    let mut h = if max == r {
        (g - b) / delta + (if g < b { 6.0 } else { 0.0 })
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    h *= 60.0;

    (h, s, l)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    if s <= 1e-6 {
        return (l, l, l);
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;

    let hk = h / 360.0;
    let tr = (hk + 1.0 / 3.0).rem_euclid(1.0);
    let tg = hk.rem_euclid(1.0);
    let tb = (hk - 1.0 / 3.0).rem_euclid(1.0);

    (
        hue_to_rgb(p, q, tr),
        hue_to_rgb(p, q, tg),
        hue_to_rgb(p, q, tb),
    )
}

fn hue_to_rgb(p: f32, q: f32, t: f32) -> f32 {
    if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 1.0 / 2.0 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    }
}

/// SilverGrain: visible, neutral film grain.
///
/// Previous implementation used a Poisson-disk of grains with `grain_radius =
/// roughness * image_width_px` — at roughness 0.12 on a 4000px image that made
/// each "grain" a ~960px blob, so what users saw was a soft muddy overlay
/// rather than grain. It also only ever darkened (factor 0.7–1.0), which a
/// downstream print LUT's S-curve then compressed away to near-nothing.
///
/// This version models grain directly in pixel space: per-pixel bilateral
/// noise (sign hashed from pixel coords so the pattern is stable across
/// re-renders) with amplitude lifted into a clearly visible range, plus an
/// optional box blur whose radius scales with `roughness` so coarse grain
/// clumps rather than staying single-pixel. The deviation is applied
/// additively in display-encoded space so it survives a subsequent print LUT.
fn apply_silvergrain(pixels: &mut [f32], width: u32, height: u32, amount: f32, roughness: f32) {
    let width = width as usize;
    let height = height as usize;
    let intensity = amount.clamp(0.0, 1.0);
    if intensity <= 0.0 {
        return;
    }

    // Amplitude maps amount (0..1) to a visibly-grainy ±deviation in display
    // space. Tuned so amount=0.05 reads as a light tooth and amount=1.0 is
    // heavy 35mm-style grain. ±0.12 at full — well above the JND on most
    // tones and survives a print LUT.
    let amplitude = 0.12 * intensity;

    // roughness (0.05..0.3 in the UI) controls grain clump size. We treat it
    // as a box-blur radius in pixels: 0.05 → ~1px (fine), 0.3 → ~5px (coarse).
    let blur_radius = (roughness * 16.0).round().max(1.0) as usize;

    // First pass: write per-pixel signed noise into a scratch buffer.
    // Using an integer hash keyed on (x,y) gives a stable, evenly distributed
    // pattern without needing a global RNG seeded per frame.
    let mut noise = vec![0.0_f32; width * height];
    for y in 0..height {
        for x in 0..width {
            // Map hash_2d_u32 (0..1) to centered bilateral noise (-1..+1).
            let n = hash_2d_u32(x as u32, y as u32) * 2.0 - 1.0;
            noise[y * width + x] = n * amplitude;
        }
    }

    // Second pass (only if blur_radius > 1): box-blur the noise field so
    // grains clump together at higher roughness — mimicking silver-halide
    // crystal clustering rather than TV-style static.
    if blur_radius > 1 {
        let mut blurred = noise.clone();
        let r = blur_radius as isize;
        // Horizontal pass.
        for y in 0..height {
            for x in 0..width {
                let mut sum = 0.0;
                let mut count = 0.0;
                let xi0 = (x as isize - r).max(0) as usize;
                let xi1 = ((x as isize + r) as usize).min(width - 1);
                for xi in xi0..=xi1 {
                    sum += noise[y * width + xi];
                    count += 1.0;
                }
                blurred[y * width + x] = sum / count;
            }
        }
        // Vertical pass (in place on `noise`, reading from `blurred`).
        for y in 0..height {
            for x in 0..width {
                let mut sum = 0.0;
                let mut count = 0.0;
                let yi0 = (y as isize - r).max(0) as usize;
                let yi1 = ((y as isize + r) as usize).min(height - 1);
                for yi in yi0..=yi1 {
                    sum += blurred[yi * width + x];
                    count += 1.0;
                }
                noise[y * width + x] = sum / count;
            }
        }

        // Averaging independent per-pixel deviations shrinks their amplitude
        // by roughly the window size (variance ~ 1/w^2), so without
        // correction higher roughness would make the grain clump *and*
        // nearly disappear. Renormalize back to the noise field's target
        // standard deviation so `roughness` only changes clump size, not
        // how visible the grain is.
        let mean: f32 = noise.iter().sum::<f32>() / noise.len() as f32;
        let variance: f32 = noise
            .iter()
            .map(|v| {
                let d = v - mean;
                d * d
            })
            .sum::<f32>()
            / noise.len() as f32;
        let std = variance.sqrt();
        let target_std = amplitude / 3.0_f32.sqrt(); // std of a uniform ±amplitude field
        if std > 1e-8 {
            let scale = target_std / std;
            for v in noise.iter_mut() {
                *v *= scale;
            }
        }
    }

    // Third pass: apply the (possibly blurred) noise field as an additive,
    // luminance-aware deviation. We attenuate the deviation in deep shadows
    // and near-clipped highlights so grain doesn't chatter on pure black or
    // pure white — matches photographic film behaviour.
    // Grain runs after AgX, on display-referred Rec.709 pixels.
    let lum_weights: [f32; 3] = [0.2126, 0.7152, 0.0722];
    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) * 3;
            let n = noise[y * width + x];
            let r = pixels[i];
            let g = pixels[i + 1];
            let b = pixels[i + 2];
            let luma = r * lum_weights[0] + g * lum_weights[1] + b * lum_weights[2];

            // Suppression curve: full strength at luma 0.5, tapering to ~0
            // at 0 and 1. A smoothstep-style mask avoids hard cutoffs.
            let mask = (luma * (1.0 - luma) * 4.0).clamp(0.0, 1.0);
            let dev = n * mask;

            pixels[i] = (r + dev).clamp(0.0, 1.0);
            pixels[i + 1] = (g + dev).clamp(0.0, 1.0);
            pixels[i + 2] = (b + dev).clamp(0.0, 1.0);
        }
    }
}

const LOGC3_CUT: f32 = 0.010591;
const LOGC3_A: f32 = 5.555556;
const LOGC3_B: f32 = 0.052272;
const LOGC3_C: f32 = 0.247190;
const LOGC3_D: f32 = 0.385537;
const LOGC3_E: f32 = 5.367655;
const LOGC3_F: f32 = 0.092809;

#[inline]
fn logc3_encode(x: f32) -> f32 {
    if x > LOGC3_CUT {
        LOGC3_C * (LOGC3_A * x + LOGC3_B).log10() + LOGC3_D
    } else {
        LOGC3_E * x + LOGC3_F
    }
}


fn load_lut_stack(lut_layers: &[LutLayer], luts_dir: &Path) -> Vec<(Arc<Cube>, f32)> {
    lut_layers
        .iter()
        .filter_map(|layer| {
            let name = layer.name.trim();
            if name.is_empty() || layer.opacity <= 0.0 {
                return None;
            }

            let candidates = [
                luts_dir.join(format!("{name}.cube")),
                luts_dir.join(format!("{name}.CUBE")),
                luts_dir.join(name),
            ];

            for path in candidates {
                if path.is_file() {
                    return match Cube::load(&path) {
                        Ok(cube) => Some((Arc::new(cube), layer.opacity.clamp(0.0, 1.0))),
                        Err(e) => {
                            eprintln!("LUT '{}' skipped: {e:#}", layer.name);
                            None
                        }
                    };
                }
            }

            eprintln!("LUT '{}' skipped: file not found", layer.name);
            None
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every field a band mixer names must actually exist on `Recipe`, and
    /// indexed ones must be in range. The frontend writes these ids into a
    /// plain JSON object, so a typo in the `format!` that builds them would
    /// not fail anywhere — it would quietly create a field nothing reads,
    /// and the slider would just do nothing.
    #[test]
    fn band_mixer_fields_all_exist_on_the_recipe() {
        let recipe = serde_json::to_value(Recipe::default()).expect("Recipe serializes");
        let obj = recipe.as_object().expect("Recipe is a JSON object");

        let mut checked = 0;
        for group in RapidEngine.control_groups() {
            for control in group.controls {
                let EngineControl::BandMixer { label, bands, channels } = control else {
                    continue;
                };
                for band in &bands {
                    assert_eq!(
                        band.fields.len(),
                        channels.len(),
                        "{label}/{}: one field per channel",
                        band.label
                    );
                    for field in &band.fields {
                        let value = obj.get(&field.id).unwrap_or_else(|| {
                            panic!("{label}/{}: no Recipe field `{}`", band.label, field.id)
                        });
                        if let Some(i) = field.index {
                            let arr = value.as_array().unwrap_or_else(|| {
                                panic!("`{}` is indexed but not an array", field.id)
                            });
                            assert!(
                                i < arr.len(),
                                "`{}`[{i}] is past the end of a {}-long field",
                                field.id,
                                arr.len()
                            );
                        }
                        checked += 1;
                    }
                }
            }
        }
        // 8 hue bands × 3 + 3 zones × 3.
        assert_eq!(checked, 33, "both mixers should have been reached");
    }

    /// The gains must land on the Planckian locus, not on a straight line.
    /// Reference values computed independently from the CIE fit and the
    /// pipeline's own sRGB→ProPhoto matrix.
    #[test]
    fn temperature_follows_the_planckian_locus() {
        for (slider, want_r, want_b) in [
            (-50.0f32, 0.881f32, 1.598f32), // 3750 K
            (-25.0, 0.955, 1.208),          // 4625 K
            (37.5, 1.043, 0.792),           // 7188 K — the setting Francis used
            (100.0, 1.066, 0.644),          // 10000 K
        ] {
            let (r, b) = temperature_gains(slider);
            assert!(
                (r - want_r).abs() < 0.01 && (b - want_b).abs() < 0.01,
                "slider {slider}: got R {r:.3} B {b:.3}, want R {want_r:.3} B {want_b:.3}"
            );
        }
        assert_eq!(temperature_gains(0.0), (1.0, 1.0), "neutral must be untouched");
    }

    /// The failure Francis actually reported: warming pushed the image toward
    /// magenta instead of toward yellow. On the locus, green sits ABOVE the
    /// midpoint of red and blue when warming and well below it when cooling;
    /// the old straight-line model pinned it to the midpoint at every
    /// temperature, which reads as magenta one way and green the other.
    #[test]
    fn warming_does_not_drift_magenta() {
        let midpoint_offset = |slider: f32| {
            let (r, b) = temperature_gains(slider);
            1.0 - (r + b) / 2.0 // green is 1.0 by normalisation
        };
        assert!(
            midpoint_offset(37.5) > 0.02,
            "warming must leave green above the R/B midpoint, got {:+.3}",
            midpoint_offset(37.5)
        );
        assert!(
            midpoint_offset(-50.0) < -0.1,
            "cooling must leave green well below it, got {:+.3}",
            midpoint_offset(-50.0)
        );
        // And the direction itself must stay right: warm = more red than blue.
        let (r, b) = temperature_gains(37.5);
        assert!(r > 1.0 && b < 1.0, "warming must raise red and lower blue");
        let (r, b) = temperature_gains(-50.0);
        assert!(r < 1.0 && b > 1.0, "cooling must lower red and raise blue");
    }

    /// End-to-end, which is where the complaint lived: the gains above are
    /// only right if they survive the rest of the pipeline. A neutral grey
    /// developed at any temperature must stay on the neutral axis — warm or
    /// cool in the red/blue sense, never tinted green or magenta.
    ///
    /// Measured on the old model this read -0.034 at 7188 K and +0.080 at
    /// 3750 K: a visible magenta cast one way and a green one the other.
    #[test]
    fn a_developed_neutral_never_drifts_green_or_magenta() {
        for slider in [-50.0f32, -25.0, 0.0, 37.5, 100.0] {
            let input = ImageBuf::from_data(1, 1, vec![0.18, 0.18, 0.18]);
            let mut recipe = Recipe::default();
            recipe.engine = "rapid".to_string();
            recipe.temperature = slider;
            let out = develop_rapid(&input, &recipe, Path::new(""));
            let (r, g, b) = (out.data[0], out.data[1], out.data[2]);
            let drift = g - (r + b) / 2.0;
            assert!(
                drift.abs() < 0.02,
                "temperature {slider}: green sits {drift:+.4} off the R/B midpoint \
                 (R {r:.4} G {g:.4} B {b:.4}) — that reads as a colour cast"
            );
            // The control must still do its job.
            if slider > 0.0 {
                assert!(r > b, "warming must leave red above blue");
            } else if slider < 0.0 {
                assert!(b > r, "cooling must leave blue above red");
            }
        }
    }

    /// `PROPHOTO_LUMA` is a precomputed shortcut for "convert to Rec.709,
    /// then take its luma". If either the matrix or the constant is ever
    /// edited alone they stop meaning the same thing, and nothing else would
    /// notice — greys agree under any weights that sum to 1.
    #[test]
    fn luma_weights_match_the_conversion_matrix() {
        for (r, g, b) in [
            (0.18, 0.18, 0.18),
            (0.50, 0.05, 0.05),
            (0.02, 0.02, 0.60),
            (0.10, 0.30, 0.08),
            (0.90, 0.40, 0.10),
        ] {
            let via_matrix = luma_709(
                PROPHOTO_TO_REC709[0][0] * r + PROPHOTO_TO_REC709[0][1] * g + PROPHOTO_TO_REC709[0][2] * b,
                PROPHOTO_TO_REC709[1][0] * r + PROPHOTO_TO_REC709[1][1] * g + PROPHOTO_TO_REC709[1][2] * b,
                PROPHOTO_TO_REC709[2][0] * r + PROPHOTO_TO_REC709[2][1] * g + PROPHOTO_TO_REC709[2][2] * b,
            );
            let direct = luma(r, g, b);
            // 1e-4 absorbs the sum-to-1 normalisation above; anything larger
            // means the constant and the matrix genuinely disagree.
            assert!(
                (via_matrix - direct).abs() < 1e-4,
                "({r}, {g}, {b}): matrix route {via_matrix:.6} vs PROPHOTO_LUMA {direct:.6}"
            );
        }
        // A neutral must still land on itself, or exposure shifts everywhere.
        assert!((luma(0.5, 0.5, 0.5) - 0.5).abs() < 1e-5);
    }

    /// The reason the wrong weights were invisible for so long, pinned as a
    /// fact: on neutrals the two sets agree exactly, and they diverge only on
    /// saturated colour. A future "simplification" back to Rec.709 weights
    /// would pass every grey-based test in this file.
    #[test]
    fn luma_differs_from_rec709_only_on_saturated_colour() {
        assert!((luma(0.18, 0.18, 0.18) - luma_709(0.18, 0.18, 0.18)).abs() < 1e-6);
        let (r, g, b) = (0.02, 0.02, 0.60); // saturated blue
        assert!(
            luma_709(r, g, b) > luma(r, g, b) * 1.5,
            "Rec.709 weights should badly overstate a ProPhoto blue's luminance"
        );
    }

    /// The bands must sit where this engine actually measures the colours
    /// they are named after. Reference hues computed independently: an sRGB
    /// colour of each name, taken through the pipeline's own sRGB→ProPhoto
    /// matrix, measured with the same `rgb_to_hsl` the HSL stage uses.
    ///
    /// The old centres were the gamma-encoded sRGB angles (0/30/60/…), up to
    /// 18° away from these; "Green" pointed at 120 while foliage lands at 102.
    #[test]
    fn hue_bands_sit_on_the_colours_they_name() {
        // Linear sRGB for a saturated colour of each band's name.
        let srgb_linear: [[f32; 3]; 8] = [
            [1.0, 0.0, 0.0],      // Red
            [1.0, 0.2158605, 0.0], // Orange
            [1.0, 1.0, 0.0],      // Yellow
            [0.0, 1.0, 0.0],      // Green
            [0.0, 1.0, 1.0],      // Aqua
            [0.0, 0.0, 1.0],      // Blue
            [0.2158605, 0.0, 1.0], // Purple
            [1.0, 0.0, 1.0],      // Magenta
        ];
        const SRGB_TO_PROPHOTO: [[f32; 3]; 3] = [
            [0.5288241004, 0.3340609866, 0.1373616909],
            [0.0975294148, 0.8790074094, 0.0233981175],
            [0.0163599018, 0.1066124933, 0.8772485185],
        ];
        for (i, c) in srgb_linear.iter().enumerate() {
            let p: Vec<f32> = SRGB_TO_PROPHOTO
                .iter()
                .map(|row| (row[0] * c[0] + row[1] * c[1] + row[2] * c[2]).max(0.0))
                .collect();
            let (h, _, _) = rgb_to_hsl(p[0], p[1], p[2]);
            let d = hue_distance(h, HUE_CENTERS[i]);
            // Orange and Purple drift with saturation, so their centre is a
            // median and sits ~10° from this fully saturated probe, which is
            // one end of their span. The other six are exact.
            let tolerance = if i == 1 || i == 6 { 12.0 } else { 1.0 };
            assert!(
                d < tolerance,
                "{}: measured hue {h:.1}°, band centre {:.1}° ({d:.1}° apart)",
                HUE_BAND_LABELS[i],
                HUE_CENTERS[i]
            );
        }
    }

    /// What Francis will actually feel, on real subject matter rather than on
    /// idealised primaries.
    ///
    /// The assertion is on the WEIGHT, not on which band wins: foliage
    /// already won "Green" under the old centres, because 102° is nearer 120
    /// than 60. What was wrong was the strength. Asserting only the winner
    /// would have passed before the fix and guarded nothing — it did, when
    /// first written, which is why it says this.
    ///
    /// Sky is deliberately absent. Measured daylight sky sits near 237°,
    /// between Aqua and Blue, so it is genuinely shared between two sliders
    /// and no single-band weight threshold is honest for it.
    #[test]
    fn foliage_lands_in_green_and_sky_lands_in_blue() {
        let strongest_band = |r: f32, g: f32, b: f32| {
            let (h, _, _) = rgb_to_hsl(r, g, b);
            (0..8)
                .map(|i| (i, 1.0 - hue_distance(h, HUE_CENTERS[i]) / 45.0))
                .filter(|(_, w)| *w > 0.0)
                .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .map(|(i, w)| (HUE_BAND_LABELS[i], w))
        };
        // Foliage: sRGB(0.25, 0.45, 0.15) taken to linear ProPhoto.
        let (won, weight) = strongest_band(0.087, 0.155, 0.036).expect("a band claims it");
        assert_eq!(won, "Green");
        assert!(
            weight > 0.8,
            "Green reaches only {weight:.2} of its travel on foliage — it was 0.44 \
             under the old centres, which is what made the slider feel weak"
        );
    }

    #[test]
    fn test_rapid_exposure_rendering() {
        let input = ImageBuf::from_data(
            2,
            2,
            vec![0.1, 0.1, 0.1, 0.2, 0.2, 0.2, 0.3, 0.3, 0.3, 0.4, 0.4, 0.4],
        );
        let mut recipe = Recipe::default();
        recipe.engine = "rapid".to_string();
        recipe.exposure_ev = 1.0;

        let output = develop_rapid(&input, &recipe, Path::new(""));
        assert_eq!(output.width, 2);
        assert_eq!(output.height, 2);
        assert_eq!(output.data.len(), 12);
        assert!(output.data[0] > 0.0);
    }

    /// The default (identity) curve must leave the render byte-identical —
    /// otherwise every existing sidecar silently changes look the moment
    /// curves ship. The same recipe with a curve that pulls midtones down
    /// must then actually darken it.
    #[test]
    fn test_tone_curve_identity_is_a_no_op_and_a_curve_is_not() {
        let input = ImageBuf::from_data(
            2,
            2,
            vec![0.1, 0.1, 0.1, 0.2, 0.2, 0.2, 0.3, 0.3, 0.3, 0.4, 0.4, 0.4],
        );
        let mut recipe = Recipe::default();
        recipe.engine = "rapid".to_string();

        let baseline = develop_rapid(&input, &recipe, Path::new(""));

        // Explicitly re-stating the identity curve changes nothing.
        recipe.curve_luma = vec![[0.0, 0.0], [1.0, 1.0]];
        let unchanged = develop_rapid(&input, &recipe, Path::new(""));
        for (a, b) in baseline.data.iter().zip(unchanged.data.iter()) {
            assert!((a - b).abs() < 1e-6, "identity curve altered the render: {a} vs {b}");
        }

        // A curve that maps midtones downward has to darken the result.
        recipe.curve_luma = vec![[0.0, 0.0], [0.5, 0.25], [1.0, 1.0]];
        let darkened = develop_rapid(&input, &recipe, Path::new(""));
        let sum_before: f32 = baseline.data.iter().sum();
        let sum_after: f32 = darkened.data.iter().sum();
        assert!(
            sum_after < sum_before,
            "a downward midtone curve should darken the frame ({sum_after} vs {sum_before})"
        );
    }

    /// The GPU path is only a speedup for as long as it renders the same
    /// image as the CPU path. This is the test that makes the fallback a
    /// fallback instead of a second, slightly different look — it runs one
    /// busy recipe (every stage engaged: guidance-map stages, tone, colour
    /// wheels, zones, HSL, curves, vignette, AgX look) down both paths and
    /// compares pixel for pixel.
    ///
    /// Skips itself when there's no adapter — CI without a GPU shouldn't
    /// fail, but a machine WITH one must agree.
    #[test]
    fn gpu_and_cpu_paths_agree() {
        if !crate::rapid_gpu::available() {
            eprintln!("no GPU adapter — skipping GPU/CPU equivalence check");
            return;
        }

        // A gradient with colour variation, so hue-dependent stages (HSL
        // bands, vibrance's skin dampener, the colour wheels) actually do
        // something rather than all seeing neutral grey.
        let (w, h) = (64usize, 48usize);
        let mut data = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                let fx = x as f32 / w as f32;
                let fy = y as f32 / h as f32;
                data.push(0.02 + fx * 1.4);
                data.push(0.02 + fy * 0.9);
                data.push(0.02 + (1.0 - fx) * 0.6);
            }
        }
        let input = ImageBuf::from_data(w as u32, h as u32, data);

        let mut recipe = Recipe::default();
        recipe.engine = "rapid".to_string();
        recipe.exposure_ev = 0.4;
        recipe.brightness = 8.0;
        recipe.midtones = 5.0;
        recipe.contrast = 0.2;
        recipe.highlights = -22.0;
        recipe.shadows = 18.0;
        recipe.whites = 6.0;
        recipe.blacks = -9.0;
        recipe.clarity = 15.0;
        recipe.structure = 10.0;
        recipe.dehaze = 8.0;
        recipe.saturation = 0.15;
        recipe.vibrance = 20.0;
        recipe.temperature = 18.0;
        recipe.tint = -7.0;
        recipe.hsl_hue = vec![10.0, -5.0, 0.0, 8.0, 0.0, -12.0, 0.0, 4.0];
        recipe.hsl_sat = vec![15.0, 0.0, -10.0, 0.0, 20.0, 0.0, 0.0, -5.0];
        recipe.hsl_lum = vec![0.0, 12.0, 0.0, -8.0, 0.0, 6.0, 0.0, 0.0];
        recipe.shadows_tint = [0.05, -0.02, 0.08];
        recipe.midtones_tint = [-0.03, 0.04, 0.0];
        recipe.highlights_tint = [0.02, 0.01, -0.05];
        recipe.zone_shadows_exposure = 0.3;
        recipe.zone_shadows_contrast = 12.0;
        recipe.zone_midtones_saturation = 15.0;
        recipe.zone_highlights_contrast = -8.0;
        recipe.vignette_amount = -0.35;
        recipe.agx_look = "punchy".to_string();
        recipe.curve_luma = vec![[0.0, 0.03], [0.5, 0.55], [1.0, 0.97]];
        recipe.curve_b = vec![[0.0, 0.0], [0.5, 0.46], [1.0, 1.0]];

        let cpu = develop_rapid_with(&input, &recipe, Path::new(""), false);
        let gpu = develop_rapid_with(&input, &recipe, Path::new(""), true);

        assert_eq!(cpu.data.len(), gpu.data.len());
        let mut worst = 0.0f32;
        let mut worst_at = 0usize;
        for (i, (a, b)) in cpu.data.iter().zip(gpu.data.iter()).enumerate() {
            let d = (a - b).abs();
            if d > worst {
                worst = d;
                worst_at = i;
            }
        }
        // Both paths are f32 doing the same operations in the same order,
        // but a GPU may fuse a multiply-add or evaluate pow() to a slightly
        // different last bit. 1/512 of the output range is far below what an
        // 8-bit export can represent, and far under what a drifting port
        // would produce.
        assert!(
            worst < 0.002,
            "GPU and CPU diverged by {worst} at index {worst_at} (cpu {}, gpu {})",
            cpu.data[worst_at],
            gpu.data[worst_at]
        );
    }

    /// Not a correctness test — the measurement that justifies the GPU path
    /// existing, kept so it can be re-run after any change to either path
    /// (adding a stage to the shader, say) rather than trusting that the
    /// gain is still there. Ignored by default; run it with:
    ///   cargo test --release -p reveal-engine rapid_render_cost -- --ignored --nocapture
    /// Release matters: in a debug build the CPU loop runs unoptimized and
    /// the comparison flatters the GPU.
    #[test]
    #[ignore = "measurement, not a correctness check"]
    fn rapid_render_cost() {
        use std::time::Instant;

        let (w, h) = (2048usize, 1365usize);
        let mut data = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                data.push(0.02 + (x as f32 / w as f32) * 1.4);
                data.push(0.02 + (y as f32 / h as f32) * 0.9);
                data.push(0.3);
            }
        }
        let input = ImageBuf::from_data(w as u32, h as u32, data);
        let mut recipe = Recipe::default();
        recipe.engine = "rapid".to_string();
        // Engage the guidance-map stages; a recipe of all-defaults would
        // skip most of the work and measure nothing interesting.
        recipe.clarity = 15.0;
        recipe.shadows = 18.0;
        recipe.vibrance = 20.0;
        recipe.contrast = 0.2;

        for (label, use_gpu) in [("cpu", false), ("gpu", true)] {
            if use_gpu && !crate::rapid_gpu::available() {
                eprintln!("gpu: no adapter");
                continue;
            }
            let _ = develop_rapid_with(&input, &recipe, Path::new(""), use_gpu); // warm up
            let t = Instant::now();
            for _ in 0..5 {
                let _ = develop_rapid_with(&input, &recipe, Path::new(""), use_gpu);
            }
            eprintln!("{label}: {:?} per {w}x{h} render", t.elapsed() / 5);
        }
    }

    /// The equivalence test above runs on a 64×48 frame, which is why it
    /// couldn't catch that the device was requested with downlevel limits:
    /// those cap a storage binding at 128 MiB, i.e. ~11 MP at 12 bytes a
    /// pixel, so every full-resolution export from a modern body fell back
    /// to the CPU without a word — precisely where the GPU is worth most.
    /// 16 MP is over that old ceiling and under any real adapter's.
    #[test]
    fn gpu_handles_an_image_past_the_downlevel_limit() {
        if !crate::rapid_gpu::available() {
            eprintln!("no GPU adapter — skipping large-image check");
            return;
        }
        let (w, h) = (4800usize, 3400usize); // 16.3 MP
        let input = ImageBuf::from_data(w as u32, h as u32, vec![0.3f32; w * h * 3]);
        let mut recipe = Recipe::default();
        recipe.engine = "rapid".to_string();
        recipe.exposure_ev = 0.5;

        let out = crate::rapid_gpu::run(
            &crate::rapid_gpu::Inputs {
                width: w,
                height: h,
                data: &input.data,
                blurred: &[],
                down_w: 0,
                down_h: 0,
                w_mult: 1.0,
                exposure_factor: 2.0f32.powf(recipe.exposure_ev),
                r_temp: 1.0,
                r_tint: 1.0,
                g_tint: 1.0,
                b_temp: 1.0,
                b_tint: 1.0,
                brightness_adj: 0.0,
                saturation_adj: 0.0,
                contrast: 0.0,
                shadows: 0.0,
                blacks: 0.0,
                highlights: 0.0,
                clarity: 0.0,
                structure: 0.0,
                dehaze: 0.0,
                vibrance: 0.0,
                has_hsl: false,
                has_color_wheels: false,
                has_zones: false,
                curves: [None, None, None, None],
            },
            &recipe,
        );

        assert!(
            out.is_some(),
            "a {w}x{h} frame fell back to the CPU — the device's buffer limits \
             are probably back at downlevel defaults"
        );
    }

    #[test]
    fn test_rapid_hsl_conversion() {
        let (h, s, l) = rgb_to_hsl(1.0, 0.0, 0.0);
        assert!((h - 0.0).abs() < 1e-4 || (h - 360.0).abs() < 1e-4);
        assert!((s - 1.0).abs() < 1e-4);
        assert!((l - 0.5).abs() < 1e-4);
    }

    /// AgX must map 18% middle-grey to a mid-display neutral (~0.511) and
    /// roll white off without clipping. Pins the RapidRAW-faithful curve so
    /// the old "too dark" regression (missing ^2.4 gamma + smoothstep) can't
    /// silently return. Tolerances allow for f32 rounding vs the f64 reference.
    #[test]
    fn test_agx_midgrey_not_dark() {
        let (r, g, b) = agx_tonemap(0.18, 0.18, 0.18);
        // Neutral and centred in the display range — the old code landed ~0.2.
        assert!(
            (r - 0.511).abs() < 0.01,
            "mid-grey r = {r:.4}, expected ~0.511"
        );
        assert!(
            (g - 0.511).abs() < 0.01,
            "mid-grey g = {g:.4}, expected ~0.511"
        );
        assert!(
            (b - 0.511).abs() < 0.01,
            "mid-grey b = {b:.4}, expected ~0.511"
        );
        assert!(
            (r - g).abs() < 1e-4 && (g - b).abs() < 1e-4,
            "grey input must stay neutral"
        );
    }

    #[test]
    fn test_agx_white_rolloff_and_shadow() {
        let (wr, wg, wb) = agx_tonemap(1.0, 1.0, 1.0);
        assert!(
            wr < 1.0 && wg < 1.0 && wb < 1.0,
            "white must roll off, got ({wr}, {wg}, {wb})"
        );
        assert!(
            wr > 0.85,
            "white should stay bright after rolloff, got {wr:.4}"
        );

        let (sr, _sg, _sb) = agx_tonemap(0.02, 0.02, 0.02);
        assert!(
            sr > 0.05,
            "shadow 0.02 must not crush to black, got {sr:.4}"
        );
        assert!(sr < 0.2, "shadow 0.02 should stay deep, got {sr:.4}");
    }

    /// Saturated inputs can drive the inset matrix negative; gamut compression
    /// must keep log2 NaN-free and the output finite.
    #[test]
    fn test_agx_handles_out_of_gamut() {
        let (r, g, b) = agx_tonemap(2.0, 0.0, 0.0);
        assert!(r.is_finite() && g.is_finite() && b.is_finite());
        assert!(r >= 0.0 && g >= 0.0 && b >= 0.0);
    }

    #[test]
    fn test_zone_weights_partition_sums_to_one() {
        for luma in [0.0, 0.02, 0.09, 0.18, 0.4, 0.5, 0.6, 0.81, 1.0] {
            let (s, m, h) = zone_weights(luma, luma, luma);
            let total = s + m + h;
            assert!(
                (total - 1.0).abs() < 1e-5,
                "luma={luma}: weights ({s}, {m}, {h}) sum to {total}, expected 1.0"
            );
            assert!(s >= 0.0 && m >= 0.0 && h >= 0.0);
        }
    }

    #[test]
    fn test_zone_exposure_targets_shadows_only() {
        // Row 0: near-black pixel; row 1: near-white pixel.
        let input = ImageBuf::from_data(1, 2, vec![0.01, 0.01, 0.01, 0.9, 0.9, 0.9]);

        let mut baseline = Recipe::default();
        baseline.engine = "rapid".to_string();
        let base_out = develop_rapid(&input, &baseline, Path::new(""));

        let mut zoned = Recipe::default();
        zoned.engine = "rapid".to_string();
        zoned.zone_shadows_exposure = -1.0;
        let zoned_out = develop_rapid(&input, &zoned, Path::new(""));

        let shadow_delta = (base_out.data[0] - zoned_out.data[0]).abs();
        let highlight_delta = (base_out.data[3] - zoned_out.data[3]).abs();

        assert!(
            shadow_delta > 0.01,
            "shadows-only exposure should visibly change the dark pixel, delta={shadow_delta}"
        );
        assert!(
            highlight_delta < shadow_delta,
            "shadows-only exposure should affect the bright pixel far less than \
             the dark one (shadow_delta={shadow_delta}, highlight_delta={highlight_delta})"
        );
    }

    /// `apply_local_contrast`'s internal highlight-protection smoothstep
    /// suppresses effect near luma 0.9-1.0 — exactly where a "Highlights"
    /// zone slider needs to act. `apply_local_contrast_masked` must bypass
    /// that and honor an externally-supplied full-strength mask instead.
    #[test]
    fn test_apply_local_contrast_masked_reaches_highlights() {
        // amount is on the same raw scale the sliders hand in (percent-like,
        // tens not fractions) — apply_local_contrast_masked divides by 100
        // internally, so 50.0 here is "most of the way up the slider", not
        // an arbitrary unit-scale value.
        let (r, _, _) = apply_local_contrast_masked(0.95, 0.95, 0.95, 0.5, 50.0, 1.0);
        assert!(
            (r - 0.95).abs() > 0.01,
            "full-mask contrast boost should visibly move a near-white pixel, got r={r}"
        );

        let (r_old, _, _) = apply_local_contrast(0.95, 0.95, 0.95, 0.5, 50.0);
        assert!(
            (r_old - 0.95).abs() < (r - 0.95).abs(),
            "apply_local_contrast's internal highlight protection should suppress \
             the effect much more than the explicit full mask does"
        );
    }

    /// A single slider step (0.5, on the zone_*_contrast/clarity/structure
    /// -50..50-ish scale) used to blow straight through both branches of
    /// apply_local_contrast_masked — reported live as "way too much" from
    /// just one step off zero. Locks in that one step now reads as subtle,
    /// and a full-strength push (50.0) still lands in a sane, non-blown-out
    /// range for both the boost and the flatten direction.
    #[test]
    fn test_local_contrast_slider_scale_is_usable() {
        let (r, g, b) = (0.8f32, 0.8f32, 0.8f32);
        let t_blurred = 0.4f32; // strong local luma difference to act on

        let (r1, _, _) = apply_local_contrast_masked(r, g, b, t_blurred, 0.5, 1.0);
        assert!(
            (r1 - r).abs() < 0.02,
            "one slider step should be a subtle nudge, not a dramatic shift, got r={r1}"
        );

        let (r_boost, _, _) = apply_local_contrast_masked(r, g, b, t_blurred, 50.0, 1.0);
        assert!(
            r_boost.is_finite() && r_boost < 4.0,
            "full-strength positive contrast shouldn't blow up into an extreme multiplier, got r={r_boost}"
        );

        let (r_flat, _, _) = apply_local_contrast_masked(r, g, b, t_blurred, -50.0, 1.0);
        assert!(
            (0.0..=1.0).contains(&r_flat),
            "full-strength negative contrast (flattening toward the blurred luma) should stay in range, got r={r_flat}"
        );
    }

    #[test]
    fn test_zone_contrast_gate_builds_guidance_map() {
        // Local luma variation for a local-contrast pass to act on.
        let mut data = Vec::with_capacity(4 * 4 * 3);
        for y in 0..4 {
            for x in 0..4 {
                let v: f32 = if (x + y) % 2 == 0 { 0.3 } else { 0.35 };
                data.extend_from_slice(&[v, v, v]);
            }
        }
        let input = ImageBuf::from_data(4, 4, data);

        let mut baseline = Recipe::default();
        baseline.engine = "rapid".to_string();
        let out_base = develop_rapid(&input, &baseline, Path::new(""));

        let mut zoned = Recipe::default();
        zoned.engine = "rapid".to_string();
        // Only a zone-contrast field is set — shadows/blacks/clarity/
        // structure/dehaze all stay at their 0.0 default, so the guidance
        // map only gets built if zone-contrast is itself part of the gate.
        zoned.zone_midtones_contrast = -30.0;
        let out_zoned = develop_rapid(&input, &zoned, Path::new(""));

        assert_ne!(
            out_zoned.data, out_base.data,
            "a zone-contrast-only recipe must actually change the render output \
             (regression guard: the guidance-map gate must include zone-contrast fields)"
        );
    }
}



