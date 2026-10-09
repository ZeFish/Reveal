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
                    EngineControl::Select {
                        id: "lut_encoding".to_string(),
                        label: "Encoding".to_string(),
                        options_type: "lut_encodings".to_string(),
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
                        min: -1.0,
                        max: 1.0,
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
                        id: "zone_reach".to_string(),
                        label: "Mask reach".to_string(),
                        min: 50.0,
                        max: 100.0,
                        step: 1.0,
                        preset: false,
                    },
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
/// top of a blue sky is what Francis saw as "a lot toward magenta, not
/// warm" (2026-09-22).
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

/// Where a photo's own black and white sit on the engine's scale, in stops
/// relative to 1.0 (the sensor's white). Blacks and Shadows are defined by the
/// distance above the photo's black, Highlights and Whites by the distance
/// below its white — the way Lightroom's sliders follow the photo instead of
/// an absolute scale it may never reach. Measured once on the whole frame and
/// read by the global layer and by every zone, so a slider means the same
/// thing wherever it sits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PhotoRange {
    pub black: f32,
    pub white: f32,
}

/// The black and white the slider profiles were calibrated against: a photo
/// sitting exactly here is the old absolute behaviour.
pub(crate) const RANGE_BLACK_REF: f32 = -9.0;
pub(crate) const RANGE_WHITE_REF: f32 = 0.0;
/// Percentiles that define "the photo's black and white", so one hot or dead
/// pixel never decides where the sliders act.
const RANGE_BLACK_PERCENTILE: f64 = 0.001;
const RANGE_WHITE_PERCENTILE: f64 = 0.999;
/// What a measured range may be: a frame that is black all over or blown
/// all over must not drag the profiles somewhere absurd.
const RANGE_BLACK_LIMITS: (f32, f32) = (-12.0, -6.0);
const RANGE_WHITE_LIMITS: (f32, f32) = (-4.0, 1.0);
const RANGE_MIN_SPAN: f32 = 3.0;
const RANGE_HIST_MIN: f32 = -20.0;
const RANGE_HIST_MAX: f32 = 4.0;
const RANGE_HIST_BINS: usize = 2048;

impl PhotoRange {
    /// A photo that sits exactly on the calibration: every profile unmoved.
    pub(crate) const REFERENCE: Self = Self { black: RANGE_BLACK_REF, white: RANGE_WHITE_REF };

    /// Measure the frame's luminance (scene-linear ProPhoto, RGB triplets).
    pub(crate) fn measure(data: &[f32]) -> Self {
        use rayon::prelude::*;
        let step = (RANGE_HIST_MAX - RANGE_HIST_MIN) / RANGE_HIST_BINS as f32;
        let hist = data
            .par_chunks_exact(3 * 4096)
            .map(|block| {
                let mut h = vec![0u32; RANGE_HIST_BINS];
                for px in block.chunks_exact(3) {
                    let l = luma(px[0], px[1], px[2]);
                    let stop = if l > 0.0 { l.log2() } else { RANGE_HIST_MIN };
                    let bin = (((stop - RANGE_HIST_MIN) / step) as isize).clamp(0, RANGE_HIST_BINS as isize - 1);
                    h[bin as usize] += 1;
                }
                h
            })
            .reduce(
                || vec![0u32; RANGE_HIST_BINS],
                |mut a, b| {
                    for (x, y) in a.iter_mut().zip(b) {
                        *x += y;
                    }
                    a
                },
            );
        // The trailing pixels that don't fill a block still count.
        let mut hist = hist;
        for px in data.chunks_exact(3 * 4096).remainder().chunks_exact(3) {
            let l = luma(px[0], px[1], px[2]);
            let stop = if l > 0.0 { l.log2() } else { RANGE_HIST_MIN };
            let bin = (((stop - RANGE_HIST_MIN) / step) as isize).clamp(0, RANGE_HIST_BINS as isize - 1);
            hist[bin as usize] += 1;
        }
        let total: u64 = hist.iter().map(|&n| n as u64).sum();
        if total == 0 {
            return Self::REFERENCE;
        }
        let at = |q: f64| -> f32 {
            let target = (total as f64 * q) as u64;
            let mut acc = 0u64;
            for (i, &n) in hist.iter().enumerate() {
                acc += n as u64;
                if acc > target {
                    return RANGE_HIST_MIN + (i as f32 + 0.5) * step;
                }
            }
            RANGE_HIST_MAX
        };
        let (black, white) = (at(RANGE_BLACK_PERCENTILE), at(RANGE_WHITE_PERCENTILE));
        // Judged before the limits: a frame that is flat, or black all over,
        // has no range to follow whatever the clamps would make of it.
        if white - black < RANGE_MIN_SPAN {
            return Self::REFERENCE;
        }
        Self {
            black: black.clamp(RANGE_BLACK_LIMITS.0, RANGE_BLACK_LIMITS.1),
            white: white.clamp(RANGE_WHITE_LIMITS.0, RANGE_WHITE_LIMITS.1),
        }
    }

    /// The same range after the picture has been multiplied by `gain`.
    pub(crate) fn scaled(self, gain: f32) -> Self {
        let stops = gain.max(1e-6).log2();
        Self { black: self.black + stops, white: self.white + stops }
    }

    /// Factor that moves this photo's black onto the calibration black.
    pub(crate) fn black_scale(self) -> f32 {
        2.0f32.powf(RANGE_BLACK_REF - self.black)
    }

    /// Factor that moves this photo's white onto the calibration white.
    pub(crate) fn white_scale(self) -> f32 {
        2.0f32.powf(RANGE_WHITE_REF - self.white)
    }
}

/// Shadow/midtone/highlight membership as a per-pixel luminance-weighted crossfade. Shared by
/// the 3-way color wheels and Zone Tone Shaping so "what counts as a shadow" is defined
/// identically everywhere in this engine.
///
/// `reach` (0.5..1) is the tone, as a fraction of the sqrt-luminance scale, where the Shadows
/// mask has fallen to nothing; the Highlights mask begins at the mirror tone `1 - reach`, and the
/// Midtones mask is a tent that peaks at the middle. At 0.5 the three masks partition the tones
/// (their weights sum to exactly 1) and the shadows end where the midtones peak. Above it they
/// overlap, so a strong adjustment fades out over a longer gradient, and `mask_tail` eases the end
/// of the gradient so that a strong lift does not fold back on itself. The longer the reach, the
/// further into the far tones the mask spills over.
/// How the Shadows and Highlights masks die away. At reach 0.5 they are straight lines (the
/// partition); from reach 0.75 up the fall-off is a square, which leaves the mask with no slope at
/// its end and is what keeps a strong lift from folding back on itself there (a pure gain of up to
/// about ×9, +3.2 EV, stays in order, against ×3 for a straight line). In between, the exponent
/// glides from 1 to 2 so that no reach setting jumps.
fn mask_tail(reach: f32) -> f32 {
    (1.0 + 4.0 * (reach - 0.5)).clamp(1.0, 2.0)
}

fn zone_weights(r: f32, g: f32, b: f32, reach: f32) -> (f32, f32, f32) {
    let reach = reach.clamp(0.5, 1.0);
    let lum_linear = (luma(r, g, b)).max(0.0);
    let n = lum_linear.sqrt().min(1.0);
    let shadow_weight = (1.0 - n / reach).clamp(0.0, 1.0).powf(mask_tail(reach));
    let highlight_weight = ((n - (1.0 - reach)) / reach).clamp(0.0, 1.0).powf(mask_tail(reach));
    let midtone_weight = (1.0 - (2.0 * n - 1.0).abs()).clamp(0.0, 1.0);
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
    let amount = amount * 0.01;

    let center_luma = (luma(r, g, b)).max(0.0);
    let safe_center_luma = center_luma.max(0.0001);
    // The guidance map holds gamma-encoded luminance; bring it back to linear
    // before comparing, or a flat patch reads as "detail" proportional to
    // how dark it is.
    let safe_blurred_luma = t_blurred.max(0.0001).powf(2.2);
    let log_ratio = (safe_center_luma / safe_blurred_luma).log2();

    // When amount > 0, boost local micro-contrast (clarity/structure).
    // When amount < 0, soften micro-contrast with edge dampening (Lightroom style),
    // NEVER blending in a raw low-res Gaussian blur buffer.
    let effective_amount = if amount < 0.0 {
        let edge_dampener = 1.0 / (1.0 + log_ratio.abs() * 0.8);
        amount * edge_dampener
    } else {
        amount
    };

    let contrast_factor = 2.0f32.powf(log_ratio * effective_amount);
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

    let center_luma = (luma(r, g, b)).max(0.0);
    let shadow_protection = smoothstep(0.0, 0.03, center_luma);
    let highlight_protection = 1.0 - smoothstep(0.9, 1.0, center_luma);
    let mask = shadow_protection * highlight_protection;

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

// ---------------------------------------------------------------- adjustments

/// The ÷8 guidance map, as the stages that read it see it.
pub(crate) struct Guidance<'a> {
    pub blurred: &'a [f32],
    pub down_w: usize,
    pub down_h: usize,
}

impl Guidance<'_> {
    fn at(&self, x: usize, y: usize) -> f32 {
        get_blurred_luma(x, y, self.blurred, self.down_w, self.down_h)
    }
}

/// The sliders of one adjustment layer, as the recipe stores them. The global
/// layer reads them from the `Recipe` itself and each tonal zone from its
/// `ZoneAdjustments`; the names and the units are the same, which is the
/// point — a slider means the same thing in every layer.
pub(crate) struct AdjustValues<'a> {
    pub exposure_ev: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub temperature: f32,
    pub tint: f32,
    pub whites: f32,
    pub highlights: f32,
    pub midtones: f32,
    pub shadows: f32,
    pub brightness: f32,
    pub blacks: f32,
    pub vibrance: f32,
    pub clarity: f32,
    pub structure: f32,
    pub dehaze: f32,
    pub hsl_hue: &'a [f32],
    pub hsl_sat: &'a [f32],
    pub hsl_lum: &'a [f32],
}

/// One complete set of adjustments, resolved into the units the pixel loop
/// works in (develop_rapid used to scale these into locals; doing it here, once
/// per layer, is what lets the same code serve the global layer and each zone).
/// `rapid_gpu` packs this field for field into the shader's `Adj`.
#[derive(Clone, Debug)]
pub(crate) struct Adjust {
    pub w_mult: f32,
    pub exposure_factor: f32,
    pub r_temp: f32,
    pub r_tint: f32,
    pub g_tint: f32,
    pub b_temp: f32,
    pub b_tint: f32,
    pub brightness_adj: f32,
    pub clarity: f32,
    pub structure: f32,
    pub dehaze: f32,
    pub contrast: f32,
    pub shadows: f32,
    pub blacks: f32,
    pub highlights: f32,
    pub saturation_adj: f32,
    pub vibrance: f32,
    /// Hue, saturation and luminance offsets for the eight colour bands.
    pub hsl: [[f32; 8]; 3],
    pub has_hsl: bool,
}

impl Adjust {
    pub(crate) fn new(v: &AdjustValues) -> Self {
        let exposure_factor = 2.0f32.powf(v.exposure_ev);
        let saturation = (v.saturation + 1.0).max(0.0);
        let (r_temp, b_temp) = temperature_gains(v.temperature);

        // Tint (-100 to 100): the off-locus green/magenta axis. Unlike
        // temperature this genuinely is a simple push — "tint" is by definition
        // the deviation perpendicular to the Planckian curve.
        let tint_shift = v.tint / 100.0;
        let g_tint = (1.0 - tint_shift * 0.5).max(0.1);
        let r_tint = (1.0 + tint_shift * 0.25).max(0.1);
        let b_tint = (1.0 + tint_shift * 0.25).max(0.1);

        // Global Whites multiplier (Whites processed first)
        let whites = v.whites / 100.0;
        let w_mult = if whites != 0.0 {
            let white_level = 1.0 - whites * 0.25;
            1.0 / white_level.max(0.01)
        } else {
            1.0
        };

        let mut hsl = [[0.0f32; 8]; 3];
        for (row, src) in hsl.iter_mut().zip([v.hsl_hue, v.hsl_sat, v.hsl_lum]) {
            for (i, slot) in row.iter_mut().enumerate() {
                *slot = src.get(i).copied().unwrap_or(0.0);
            }
        }
        // Any nonzero band means the user touched the HSL matrix — the recipe
        // default ships a zeroed 8-length vec (so per-band sliders can bind
        // to it), which would otherwise make `len() >= 8` true even when unused.
        let has_hsl = hsl.iter().flatten().any(|v| *v != 0.0);

        Self {
            w_mult,
            exposure_factor,
            r_temp,
            r_tint,
            g_tint,
            b_temp,
            b_tint,
            brightness_adj: v.midtones / 100.0 + v.brightness / 100.0,
            // Percent, as `apply_local_contrast_masked` expects: it applies the
            // 0.01 itself. Dividing here too made both sliders 100x too weak.
            clarity: v.clarity,
            structure: v.structure,
            dehaze: v.dehaze / 100.0,
            contrast: v.contrast,
            shadows: v.shadows / 100.0,
            blacks: v.blacks / 100.0,
            highlights: v.highlights / 100.0,
            saturation_adj: saturation - 1.0,
            vibrance: v.vibrance / 100.0,
            hsl,
            has_hsl,
        }
    }

    pub(crate) fn from_recipe(r: &Recipe) -> Self {
        Self::new(&AdjustValues {
            exposure_ev: r.exposure_ev,
            contrast: r.contrast,
            saturation: r.saturation,
            temperature: r.temperature,
            tint: r.tint,
            whites: r.whites,
            highlights: r.highlights,
            midtones: r.midtones,
            shadows: r.shadows,
            brightness: r.brightness,
            blacks: r.blacks,
            vibrance: r.vibrance,
            clarity: r.clarity,
            structure: r.structure,
            dehaze: r.dehaze,
            hsl_hue: &r.hsl_hue,
            hsl_sat: &r.hsl_sat,
            hsl_lum: &r.hsl_lum,
        })
    }

    pub(crate) fn from_zone(z: &crate::ZoneAdjustments) -> Self {
        Self::new(&AdjustValues {
            exposure_ev: z.exposure_ev,
            contrast: z.contrast,
            saturation: z.saturation,
            temperature: z.temperature,
            tint: z.tint,
            whites: z.whites,
            highlights: z.highlights,
            midtones: z.midtones,
            shadows: z.shadows,
            brightness: z.brightness,
            blacks: z.blacks,
            vibrance: z.vibrance,
            clarity: z.clarity,
            structure: z.structure,
            dehaze: z.dehaze,
            hsl_hue: &z.hsl_hue,
            hsl_sat: &z.hsl_sat,
            hsl_lum: &z.hsl_lum,
        })
    }

    /// Whether running this layer changes nothing at all — the test that lets an
    /// untouched layer cost nothing. Uses the same thresholds as the stage
    /// guards below, so "identity" and "every stage skips itself" agree.
    pub(crate) fn is_identity(&self) -> bool {
        self.w_mult == 1.0
            && self.exposure_factor == 1.0
            && self.r_temp == 1.0
            && self.r_tint == 1.0
            && self.g_tint == 1.0
            && self.b_temp == 1.0
            && self.b_tint == 1.0
            && self.brightness_adj == 0.0
            && self.clarity == 0.0
            && self.structure == 0.0
            && self.dehaze == 0.0
            && self.contrast.abs() <= 1e-4
            && self.shadows == 0.0
            && self.blacks == 0.0
            && self.highlights == 0.0
            && self.saturation_adj.abs() <= 1e-4
            && self.vibrance == 0.0
            && !self.has_hsl
    }

    /// Whether a stage of this layer reads the guidance map.
    pub(crate) fn needs_guidance(&self) -> bool {
        self.shadows != 0.0
            || self.blacks != 0.0
            || self.clarity != 0.0
            || self.structure != 0.0
            || self.dehaze != 0.0
    }

    /// Whether a stage of this layer needs to know where the photo's black and
    /// white sit (so the frame is measured only when one of them is touched).
    pub(crate) fn needs_range(&self) -> bool {
        self.shadows != 0.0 || self.blacks != 0.0 || self.highlights != 0.0 || self.w_mult != 1.0
    }

    /// The Whites gain a pixel of luminance `lum` receives, given where the
    /// photo's white sits (`white`, stops re 1.0, after exposure). Full gain at
    /// the photo's white, fading to none six stops below it: Whites opens or
    /// closes the top of THIS photo and leaves its shadows alone, instead of
    /// being a second exposure slider.
    pub(crate) fn whites_gain(&self, lum: f32, white: f32) -> f32 {
        if self.w_mult == 1.0 {
            return 1.0;
        }
        let d = lum.max(1e-6).log2() - white;
        1.0 + (self.w_mult - 1.0) * smoothstep(-6.0, 0.0, d)
    }

    /// Where the photo's range sits once this layer's exposure and Whites have
    /// been applied — the range the later stages (and the zones) read.
    pub(crate) fn range_after(&self, range: PhotoRange) -> PhotoRange {
        let mut out = range.scaled(self.exposure_factor);
        if self.w_mult != 1.0 {
            out.white += self.w_mult.log2();
        }
        out
    }

    /// Stages 1–5: exposure and white balance, Whites, local contrast, filmic
    /// brightness, contrast, shadows and blacks, highlights. `range` is the
    /// photo's black and white as this layer receives it.
    pub(crate) fn tone(
        &self,
        c: [f32; 3],
        x: usize,
        y: usize,
        guide: &Guidance,
        range: &PhotoRange,
    ) -> [f32; 3] {
        let [in_r, in_g, in_b] = c;
        // 1. Exposure, Temp & Tint WB, then Whites (a gain weighted by how close
        // the pixel is to the photo's own white)
        let mut r = in_r * self.exposure_factor * self.r_temp * self.r_tint;
        let mut g = in_g * self.exposure_factor * self.g_tint;
        let mut b = in_b * self.exposure_factor * self.b_temp * self.b_tint;
        if self.w_mult != 1.0 {
            let gw = self.whites_gain(luma(r, g, b).max(0.0), range.scaled(self.exposure_factor).white);
            r *= gw;
            g *= gw;
            b *= gw;
        }
        let range_now = self.range_after(*range);
        let black_scale = range_now.black_scale();
        let white_scale = range_now.white_scale();

        let (clarity, structure, dehaze) = (self.clarity, self.structure, self.dehaze);
        let (contrast, shadows, blacks, highlights) =
            (self.contrast, self.shadows, self.blacks, self.highlights);

        // Clarity, Structure, Dehaze (using guidance map)
        if (clarity != 0.0 || structure != 0.0 || dehaze != 0.0) && !guide.blurred.is_empty() {
            let t_blurred = guide.at(x, y);

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

        // 2. Filmic Exposure / Brightness (recipe.midtones + recipe.brightness)
        if self.brightness_adj != 0.0 {
            let color_bright = apply_filmic_exposure([r, g, b], self.brightness_adj);
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
            // The profiles below are calibrated for a black at RANGE_BLACK_REF;
            // moving the pixel (and its neighbourhood) by the photo's own
            // offset puts the photo's black where the curves expect it. The
            // lift is a ratio, so it is the same ratio at the true luminance.
            let luma_linear = (luma(r, g, b) * black_scale).max(0.0);
            let safe_pixel_luma = luma_linear.max(0.0001);
            let t_pixel = safe_pixel_luma.powf(0.4545);

            // Lookup blurred guidance luma
            let t_blurred = guide.at(x, y) * black_scale.powf(0.4545);

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
            // Same idea as the blacks: the photo's white goes where the
            // profile expects white.
            let pixel_luma = (luma(r, g, b) * white_scale).max(0.0);
            let safe_pixel_luma = pixel_luma.max(0.0001);

            let pixel_mask_input = (safe_pixel_luma * 1.5).tanh();
            let highlight_mask = smoothstep(0.3, 0.95, pixel_mask_input);

            if highlight_mask > 0.001 {
                let (final_adjusted_r, final_adjusted_g, final_adjusted_b) = if highlights < 0.0 {
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
                    let new_luma = new_luma / white_scale; // back to the photo's own units
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

        [r, g, b]
    }

    /// Stage 7: saturation, vibrance and the HSL band matrix.
    pub(crate) fn colour(&self, c: [f32; 3]) -> [f32; 3] {
        let [mut r, mut g, mut b] = c;
        if self.saturation_adj.abs() > 1e-4 || self.vibrance != 0.0 || self.has_hsl {
            let (nr, ng, nb) = apply_vibrance(r, g, b, self.saturation_adj, self.vibrance);
            if self.has_hsl {
                let (h, mut s, mut l) = rgb_to_hsl(nr.max(0.0), ng.max(0.0), nb.max(0.0));
                let mut hue_adj = 0.0f32;
                let mut sat_adj = 0.0f32;
                let mut lum_adj = 0.0f32;

                for i in 0..8 {
                    let center = HUE_CENTERS[i];
                    let dist = hue_distance(h, center);
                    if dist < 45.0 {
                        let weight = (1.0 - dist / 45.0).max(0.0);
                        hue_adj += self.hsl[0][i] * weight;
                        sat_adj += self.hsl[1][i] * weight;
                        lum_adj += self.hsl[2][i] * weight;
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
        [r, g, b]
    }
}

/// An adjustment layer: its sliders and its four tone-curve LUTs (luma, R, G,
/// B; `None` where the curve is identity).
pub(crate) struct Layer {
    pub adjust: Adjust,
    pub curves: [Option<Vec<f32>>; 4],
}

impl Layer {
    fn new(adjust: Adjust, curves: [&Vec<[f32; 2]>; 4]) -> Self {
        // Tone-curve LUTs are built once per render, not per pixel — an
        // identity curve (the default) builds nothing at all, so a recipe that
        // never touched a curve pays a single Option check per pixel.
        let build = |pts: &Vec<[f32; 2]>| {
            // A zone's curves default to empty, which also means "no curve".
            if pts.is_empty() || crate::curves::is_identity(pts) {
                None
            } else {
                crate::curves::build_lut(pts)
            }
        };
        Self { adjust, curves: curves.map(build) }
    }

    pub(crate) fn has_curves(&self) -> bool {
        self.curves.iter().any(Option::is_some)
    }

    /// Whether this layer does anything. An untouched layer costs nothing: no
    /// zone weights, no extra pass, no extra uniforms read.
    pub(crate) fn is_active(&self) -> bool {
        !self.adjust.is_identity() || self.has_curves()
    }

    /// The layer's curves on display-referred 0..1 values.
    fn apply_curves(&self, rgb: [f32; 3]) -> [f32; 3] {
        let [mut r, mut g, mut b] = rgb.map(|v| v.clamp(0.0, 1.0));
        if let Some(lut) = &self.curves[0] {
            r = crate::curves::sample(lut, r);
            g = crate::curves::sample(lut, g);
            b = crate::curves::sample(lut, b);
        }
        if let Some(lut) = &self.curves[1] {
            r = crate::curves::sample(lut, r);
        }
        if let Some(lut) = &self.curves[2] {
            g = crate::curves::sample(lut, g);
        }
        if let Some(lut) = &self.curves[3] {
            b = crate::curves::sample(lut, b);
        }
        [r, g, b]
    }
}

/// The global layer and the three tonal-zone layers (shadows, midtones,
/// highlights). A zone is a luminosity mask carrying a full set of the same
/// adjustments: it is applied on top of the global result, blended by how much
/// each pixel belongs to the zone.
pub(crate) struct Layers {
    pub global: Layer,
    pub zones: [Layer; 3],
}

impl Layers {
    pub(crate) fn from_recipe(r: &Recipe) -> Self {
        let zone = |z: &crate::ZoneAdjustments| {
            Layer::new(
                Adjust::from_zone(z),
                [&z.curve_luma, &z.curve_r, &z.curve_g, &z.curve_b],
            )
        };
        Self {
            global: Layer::new(
                Adjust::from_recipe(r),
                [&r.curve_luma, &r.curve_r, &r.curve_g, &r.curve_b],
            ),
            zones: [zone(&r.zone_shadows), zone(&r.zone_midtones), zone(&r.zone_highlights)],
        }
    }

    pub(crate) fn zone_active(&self) -> [bool; 3] {
        [self.zones[0].is_active(), self.zones[1].is_active(), self.zones[2].is_active()]
    }

    /// Whether any layer reads the photo's black and white.
    pub(crate) fn needs_range(&self) -> bool {
        self.global.adjust.needs_range() || self.zones.iter().any(|z| z.adjust.needs_range())
    }

    /// Whether any layer reads the guidance map (so it is worth building).
    fn needs_guidance(&self) -> bool {
        self.global.adjust.needs_guidance()
            || self.zones.iter().any(|z| z.adjust.needs_guidance())
    }
}

/// The ÷8 gamma-encoded luminance map that clarity, structure, dehaze and the
/// shadows/blacks detail recovery sample, built from the input as the global
/// layer would expose it.
fn build_guidance(
    input: &ImageBuf,
    width: usize,
    height: usize,
    a: &Adjust,
    range: &PhotoRange,
) -> (Vec<f32>, usize, usize) {
    let down_w = (width / 8).max(1);
    let down_h = (height / 8).max(1);
    let mut downsampled = vec![0.0f32; down_w * down_h];

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
                    r_sum += input.data[idx];
                    g_sum += input.data[idx + 1];
                    b_sum += input.data[idx + 2];
                }
            }
            let mut r_avg = (r_sum / count) * a.exposure_factor * a.r_temp * a.r_tint;
            let mut g_avg = (g_sum / count) * a.exposure_factor * a.g_tint;
            let mut b_avg = (b_sum / count) * a.exposure_factor * a.b_temp * a.b_tint;
            // Whites, weighted by the photo's own white, as `tone` does.
            let gw = a.whites_gain(luma(r_avg, g_avg, b_avg).max(0.0), range.scaled(a.exposure_factor).white);
            r_avg *= gw;
            g_avg *= gw;
            b_avg *= gw;

            // Filmic Exposure (using recipe.midtones + recipe.brightness)
            let [r_proc, g_proc, b_proc] = if a.brightness_adj != 0.0 {
                apply_filmic_exposure([r_avg, g_avg, b_avg], a.brightness_adj)
            } else {
                [r_avg, g_avg, b_avg]
            };

            let luma_linear = (luma(r_proc, g_proc, b_proc)).max(0.0);
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
        crate::lut::apply_stack_encoded(&mut work_input, &pre_luts, recipe.encoding());
    }

    // The global layer plus the three tonal zones. Only the layers that carry a
    // modification are run: with nothing touched but the global sliders, the
    // zone weights are never computed and the render costs what it always did.
    // The format is used once. Without a Pre-Lut the render leaves in it, for a Post-Lut built on
    // that format. With a Pre-Lut stack the format was consumed on the way in, to feed the stack,
    // and what the stack returned is the working signal: it is graded as it is and leaves as it
    // is, for the Post-Lut to take from there. (Encoding it a second time on the way out made a
    // look LUT's already-finished picture come out as a flat log image.)
    let output_encoding = if !pre_luts.is_empty() && recipe.encoding() != crate::LutEncoding::Display {
        crate::LutEncoding::Linear
    } else {
        recipe.encoding()
    };
    let layers = Layers::from_recipe(recipe);
    let zone_active = layers.zone_active();
    let any_zone = zone_active.iter().any(|a| *a);

    let shadows_tint = recipe.shadows_tint;
    let midtones_tint = recipe.midtones_tint;
    let highlights_tint = recipe.highlights_tint;
    let has_color_wheels = shadows_tint != [0.0, 0.0, 0.0]
        || midtones_tint != [0.0, 0.0, 0.0]
        || highlights_tint != [0.0, 0.0, 0.0];

    // Build the blurred guidance map if any layer's shadows, blacks, clarity,
    // structure or dehaze are active.
    // Where this photo's own black and white sit. Measured once on the whole
    // frame, before any slider: the global layer reads it directly, and the
    // zones read the same range as the global layer leaves it.
    let range = if layers.needs_range() {
        PhotoRange::measure(&work_input.data)
    } else {
        PhotoRange::REFERENCE
    };
    let zone_range = layers.global.adjust.range_after(range);

    let (blurred, down_w, down_h) = if layers.needs_guidance() {
        build_guidance(&work_input, width, height, &layers.global.adjust, &range)
    } else {
        (Vec::new(), 0, 0)
    };
    let guide = Guidance { blurred: &blurred, down_w, down_h };

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
                layers: &layers,
                zone_active,
                has_color_wheels,
                output_encoding,
                range,
                zone_range,
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

            // 1–5. Global layer: white point, exposure, white balance, local
            // contrast, brightness, contrast, shadows/blacks, highlights.
            let mut c = layers.global.adjust.tone(
                [
                    work_input.data[in_idx],
                    work_input.data[in_idx + 1],
                    work_input.data[in_idx + 2],
                ],
                x_coord,
                y_coord,
                &guide,
                &range,
            );

            // 6. 3-Way Color Wheels
            if has_color_wheels {
                let (shadow_weight, midtone_weight, highlight_weight) = zone_weights(c[0], c[1], c[2], recipe.zone_reach / 100.0);

                c[0] += shadows_tint[0] * shadow_weight * 0.2
                    + midtones_tint[0] * midtone_weight * 0.2
                    + highlights_tint[0] * highlight_weight * 0.2;
                c[1] += shadows_tint[1] * shadow_weight * 0.2
                    + midtones_tint[1] * midtone_weight * 0.2
                    + highlights_tint[1] * highlight_weight * 0.2;
                c[2] += shadows_tint[2] * shadow_weight * 0.2
                    + midtones_tint[2] * midtone_weight * 0.2
                    + highlights_tint[2] * highlight_weight * 0.2;
            }

            // 7. Saturation, Vibrance & HSL Matrix (global layer)
            c = layers.global.adjust.colour(c);

            // 7a. Tonal zones: shadows, midtones and highlights are luminosity
            // masks, each carrying the same set of adjustments as the global
            // layer (the Lightroom luminosity-range masks). Each runs on top of
            // the global result and is blended in by how much the pixel belongs
            // to the zone. The weights are fixed from the global result, so a
            // zone's own change never moves the mask of the next one.
            let mut zw = [0.0f32; 3];
            if any_zone {
                let (ws, wm, wh) = zone_weights(c[0], c[1], c[2], recipe.zone_reach / 100.0);
                zw = [ws, wm, wh];
                for i in 0..3 {
                    if !zone_active[i] || zw[i] <= 0.0 {
                        continue;
                    }
                    let a = &layers.zones[i].adjust;
                    let adjusted = a.colour(a.tone(c, x_coord, y_coord, &guide, &zone_range));
                    for k in 0..3 {
                        c[k] += (adjusted[k] - c[k]) * zw[i];
                    }
                }
            }
            let [mut r, mut g, mut b] = c;

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

            let encoding = output_encoding;
            let mut out = if encoding != crate::LutEncoding::Display {
                // The signal leaves in the chosen format, without the tone map: it feeds the
                // Post-Lut stack (a print LUT built for Cineon or LogC3), and on its own it is
                // supposed to look flat and dark.
                [
                    encoding.encode(r_709),
                    encoding.encode(g_709),
                    encoding.encode(b_709),
                ]
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

                [agx_r, agx_g, agx_b]
            };

            // Tone curves: on 0..1 values (display-referred or log-encoded).
            // The global layer's, then each zone's, blended in by the zone weight.
            if layers.global.has_curves() {
                out = layers.global.apply_curves(out);
            }
            for i in 0..3 {
                if zone_active[i] && zw[i] > 0.0 && layers.zones[i].has_curves() {
                    let curved = layers.zones[i].apply_curves(out);
                    for k in 0..3 {
                        out[k] += (curved[k] - out[k]) * zw[i];
                    }
                }
            }

            pixel[0] = out[0].clamp(0.0, 1.0);
            pixel[1] = out[1].clamp(0.0, 1.0);
            pixel[2] = out[2].clamp(0.0, 1.0);
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

/// High-quality 32-bit hash with full avalanche to eliminate all spatial lattice/grid artifacts.
/// Permutation table for Stefan Gustavson's Simplex Noise, doubled to 512 entries
/// to avoid wrapping computations.
#[rustfmt::skip]
const SIMPLEX_PERM: [usize; 512] = [
    151, 160, 137, 91,  90,  15,  131, 13,  201, 95,  96,  53,  194, 233, 7,   225, 140, 36,  103, 30,
    69,  142, 8,   99,  37,  240, 21,  10,  23,  190, 6,   148, 247, 120, 234, 75,  0,   26,  197, 62,
    94,  252, 219, 203, 117, 35,  11,  32,  57,  177, 33,  88,  237, 149, 56,  87,  174, 20,  125, 136,
    171, 168, 68,  175, 74,  165, 71,  134, 139, 48,  27,  166, 77,  146, 158, 231, 83,  111, 229, 122,
    60,  211, 133, 230, 220, 105, 92,  41,  55,  46,  245, 40,  244, 102, 143, 54,  65,  25,  63,  161,
    1,   216, 80,  73,  209, 76,  132, 187, 208, 89,  18,  169, 200, 196, 135, 130, 116, 188, 159, 86,
    164, 100, 109, 198, 173, 186, 3,   64,  52,  217, 226, 250, 124, 123, 5,   202, 38,  147, 118, 126,
    255, 82,  85,  212, 207, 206, 59,  227, 47,  16,  58,  17,  182, 189, 28,  42,  223, 183, 170, 213,
    119, 248, 152, 2,   44,  154, 163, 70,  221, 153, 101, 155, 167, 43,  172, 9,   129, 22,  39,  253,
    19,  98,  108, 110, 79,  113, 224, 232, 178, 185, 112, 104, 218, 246, 97,  228, 251, 34,  242, 193,
    238, 210, 144, 12,  191, 179, 162, 241, 81,  51,  145, 235, 249, 14,  239, 107, 49,  192, 214, 31,
    181, 199, 106, 157, 184, 84,  204, 176, 115, 121, 50,  45,  127, 4,   150, 254, 138, 236, 205, 93,
    222, 114, 67,  29,  24,  72,  243, 141, 128, 195, 78,  66,  215, 61,  156, 180,
    151, 160, 137, 91,  90,  15,  131, 13,  201, 95,  96,  53,  194, 233, 7,   225, 140, 36,  103, 30,
    69,  142, 8,   99,  37,  240, 21,  10,  23,  190, 6,   148, 247, 120, 234, 75,  0,   26,  197, 62,
    94,  252, 219, 203, 117, 35,  11,  32,  57,  177, 33,  88,  237, 149, 56,  87,  174, 20,  125, 136,
    171, 168, 68,  175, 74,  165, 71,  134, 139, 48,  27,  166, 77,  146, 158, 231, 83,  111, 229, 122,
    60,  211, 133, 230, 220, 105, 92,  41,  55,  46,  245, 40,  244, 102, 143, 54,  65,  25,  63,  161,
    1,   216, 80,  73,  209, 76,  132, 187, 208, 89,  18,  169, 200, 196, 135, 130, 116, 188, 159, 86,
    164, 100, 109, 198, 173, 186, 3,   64,  52,  217, 226, 250, 124, 123, 5,   202, 38,  147, 118, 126,
    255, 82,  85,  212, 207, 206, 59,  227, 47,  16,  58,  17,  182, 189, 28,  42,  223, 183, 170, 213,
    119, 248, 152, 2,   44,  154, 163, 70,  221, 153, 101, 155, 167, 43,  172, 9,   129, 22,  39,  253,
    19,  98,  108, 110, 79,  113, 224, 232, 178, 185, 112, 104, 218, 246, 97,  228, 251, 34,  242, 193,
    238, 210, 144, 12,  191, 179, 162, 241, 81,  51,  145, 235, 249, 14,  239, 107, 49,  192, 214, 31,
    181, 199, 106, 157, 184, 84,  204, 176, 115, 121, 50,  45,  127, 4,   150, 254, 138, 236, 205, 93,
    222, 114, 67,  29,  24,  72,  243, 141, 128, 195, 78,  66,  215, 61,  156, 180,
];

const SIMPLEX_GRAD3: [[f32; 3]; 12] = [
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0],
    [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0],
    [0.0, -1.0, 1.0],
    [0.0, 1.0, -1.0],
    [0.0, -1.0, -1.0],
];

/// 3D Simplex noise evaluation in f32.
/// Evaluates continuous gradients on an equilateral triangular/tetrahedral simplex mesh,
/// which is mathematically isotropic and strictly prevents any orthogonal Cartesian
/// grid, line, or weave artifacts.
#[inline(always)]
fn simplex3d(xin: f32, yin: f32, zin: f32) -> f32 {
    const F3: f32 = 1.0 / 3.0;
    const G3: f32 = 1.0 / 6.0;

    let s = (xin + yin + zin) * F3;
    let i = (xin + s).floor() as i32;
    let j = (yin + s).floor() as i32;
    let k = (zin + s).floor() as i32;

    let t = (i + j + k) as f32 * G3;
    let x0 = xin - (i as f32 - t);
    let y0 = yin - (j as f32 - t);
    let z0 = zin - (k as f32 - t);

    let (i1, j1, k1, i2, j2, k2) = if x0 >= y0 {
        if y0 >= z0 {
            (1, 0, 0, 1, 1, 0)
        } else if x0 >= z0 {
            (1, 0, 0, 1, 0, 1)
        } else {
            (0, 0, 1, 1, 0, 1)
        }
    } else if y0 < z0 {
        (0, 0, 1, 0, 1, 1)
    } else if x0 < z0 {
        (0, 1, 0, 0, 1, 1)
    } else {
        (0, 1, 0, 1, 1, 0)
    };

    let x1 = x0 - i1 as f32 + G3;
    let y1 = y0 - j1 as f32 + G3;
    let z1 = z0 - k1 as f32 + G3;

    let x2 = x0 - i2 as f32 + 2.0 * G3;
    let y2 = y0 - j2 as f32 + 2.0 * G3;
    let z2 = z0 - k2 as f32 + 2.0 * G3;

    let x3 = x0 - 1.0 + 3.0 * G3;
    let y3 = y0 - 1.0 + 3.0 * G3;
    let z3 = z0 - 1.0 + 3.0 * G3;

    let ii = (i & 255) as usize;
    let jj = (j & 255) as usize;
    let kk = (k & 255) as usize;

    let gi0 = SIMPLEX_PERM[ii + SIMPLEX_PERM[jj + SIMPLEX_PERM[kk]]] % 12;
    let gi1 = SIMPLEX_PERM[ii + i1 as usize + SIMPLEX_PERM[jj + j1 as usize + SIMPLEX_PERM[kk + k1 as usize]]] % 12;
    let gi2 = SIMPLEX_PERM[ii + i2 as usize + SIMPLEX_PERM[jj + j2 as usize + SIMPLEX_PERM[kk + k2 as usize]]] % 12;
    let gi3 = SIMPLEX_PERM[ii + 1 + SIMPLEX_PERM[jj + 1 + SIMPLEX_PERM[kk + 1]]] % 12;

    #[inline(always)]
    fn contrib(x: f32, y: f32, z: f32, gi: usize) -> f32 {
        let t = 0.6 - x * x - y * y - z * z;
        if t <= 0.0 {
            0.0
        } else {
            let t2 = t * t;
            let g = SIMPLEX_GRAD3[gi];
            t2 * t2 * (g[0] * x + g[1] * y + g[2] * z)
        }
    }

    32.0 * (contrib(x0, y0, z0, gi0)
        + contrib(x1, y1, z1, gi1)
        + contrib(x2, y2, z2, gi2)
        + contrib(x3, y3, z3, gi3))
}

/// Realistic film grain synthesis using multi-octave Simplex Noise.
/// Based on Newson et al. (2017) ("Realistic film grain rendering with simplex noise")
/// and Darktable's grain module (`iop/grain.c`).
///
/// Combines 3 octaves calibrated directly against physical silver halide film scans:
/// - Octave 0: frequency 0.4910, amplitude 0.2340
/// - Octave 1: frequency 0.9441, amplitude 0.7850
/// - Octave 2: frequency 1.7280, amplitude 1.2150
///
/// This produces a 100% isotropic spatial power spectrum identical to true film emulsion,
/// eliminating all horizontal/vertical lines, grids, screen-door effects, and moiré banding.
fn apply_silvergrain(pixels: &mut [f32], width: u32, height: u32, amount: f32, roughness: f32) {
    let width = width as usize;
    let height = height as usize;
    let intensity = amount.clamp(0.0, 1.0);
    if intensity <= 0.0 || width == 0 || height == 0 {
        return;
    }

    // Frequencies and amplitudes calibrated to real film grain scans (Newson et al. 2017)
    const OCTAVE_F: [f32; 3] = [0.4910, 0.9441, 1.7280];
    const OCTAVE_A: [f32; 3] = [0.2340, 0.7850, 1.2150];

    // Roughness controls grain crystal cluster scale:
    // 0.0 -> 0.85 (ultra-fine 35mm grain, ISO 50/100)
    // 1.0 -> 3.00 (coarse, pushed vintage film grain, ISO 1600/3200)
    let zoom = 0.85 + roughness.clamp(0.0, 1.0) * 2.15;

    // Amplitude scaling: unit amplitude yields subtle organic texture,
    // up to rich vintage grain at 1.0.
    let amplitude = 0.055 * intensity / 0.62;

    let lum_weights: [f32; 3] = [0.2126, 0.7152, 0.0722];

    pixels.par_chunks_exact_mut(width * 3).enumerate().for_each(|(y, row)| {
        let y_f = y as f32;
        for x in 0..width {
            let x_f = x as f32;

            // Multi-octave simplex noise evaluation
            let mut noise = 0.0f32;
            for oct in 0..3 {
                let f = OCTAVE_F[oct] / zoom;
                let a = OCTAVE_A[oct];
                noise += simplex3d(x_f * f, y_f * f, oct as f32 * 2.37) * a;
            }

            let i = x * 3;
            let r = row[i];
            let g = row[i + 1];
            let b = row[i + 2];
            let luma = r * lum_weights[0] + g * lum_weights[1] + b * lum_weights[2];

            // Emulsion response: grain variance peaks in midtones (0.18..0.45)
            // and rolls off smoothly toward pure highlights and deep blacks.
            let mask = (4.0 * luma * (1.0 - luma)).clamp(0.0, 1.0).sqrt();
            let dev = noise * amplitude * mask;

            row[i] = (r + dev).clamp(0.0, 1.0);
            row[i + 1] = (g + dev).clamp(0.0, 1.0);
            row[i + 2] = (b + dev).clamp(0.0, 1.0);
        }
    });
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
    use crate::ZoneAdjustments;

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
        // 8 hue bands × 3 channels.
        assert_eq!(checked, 24, "the colour mixer should have been reached");
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
        // Every adjustment in every zone, curves and colour bands included: the
        // zones run the same code as the global layer, and this is what holds the
        // shader's copy of it to the CPU's.
        recipe.zone_shadows = ZoneAdjustments {
            exposure_ev: 0.3,
            contrast: 0.2,
            brightness: 10.0,
            temperature: 12.0,
            tint: -6.0,
            saturation: -0.2,
            vibrance: 15.0,
            whites: -5.0,
            highlights: -10.0,
            midtones: 6.0,
            shadows: 20.0,
            blacks: -8.0,
            clarity: 12.0,
            structure: 8.0,
            dehaze: 5.0,
            hsl_hue: vec![0.0, 6.0, 0.0, -4.0, 0.0, 0.0, 9.0, 0.0],
            hsl_sat: vec![10.0, 0.0, 0.0, 0.0, -12.0, 0.0, 0.0, 0.0],
            hsl_lum: vec![0.0, 0.0, 8.0, 0.0, 0.0, -6.0, 0.0, 0.0],
            curve_luma: vec![[0.0, 0.02], [0.5, 0.52], [1.0, 1.0]],
            ..ZoneAdjustments::default()
        };
        recipe.zone_midtones = ZoneAdjustments {
            contrast: -0.15,
            saturation: 0.25,
            temperature: -10.0,
            tint: 5.0,
            clarity: -8.0,
            hsl_sat: vec![0.0, 8.0, 0.0, 0.0, 0.0, 0.0, -9.0, 0.0],
            curve_g: vec![[0.0, 0.0], [0.5, 0.55], [1.0, 1.0]],
            ..ZoneAdjustments::default()
        };
        recipe.zone_highlights = ZoneAdjustments {
            exposure_ev: -0.4,
            highlights: -30.0,
            whites: 10.0,
            saturation: -0.3,
            vibrance: -10.0,
            dehaze: -6.0,
            curve_r: vec![[0.0, 0.0], [0.6, 0.55], [1.0, 0.98]],
            curve_b: vec![[0.0, 0.0], [0.5, 0.47], [1.0, 1.0]],
            ..ZoneAdjustments::default()
        };
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

    /// The shader carries its own copy of the three formats that leave without the tone map;
    /// each must agree with the CPU's.
    #[test]
    fn gpu_and_cpu_agree_on_every_output_format() {
        if !crate::rapid_gpu::available() {
            eprintln!("no GPU adapter — skipping GPU/CPU equivalence check");
            return;
        }
        let (w, h) = (32usize, 24usize);
        let mut data = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                data.push(0.002 + (x as f32 / w as f32).powi(2) * 3.0);
                data.push(0.002 + (y as f32 / h as f32) * 0.8);
                data.push(0.02 + (1.0 - x as f32 / w as f32) * 0.5);
            }
        }
        let input = ImageBuf::from_data(w as u32, h as u32, data);
        for encoding in [crate::LutEncoding::LogC3, crate::LutEncoding::Cineon, crate::LutEncoding::Linear] {
            let mut recipe = rapid_recipe();
            recipe.lut_encoding = encoding;
            let cpu = develop_rapid_with(&input, &recipe, Path::new(""), false);
            let gpu = develop_rapid_with(&input, &recipe, Path::new(""), true);
            let worst = cpu.data.iter().zip(gpu.data.iter()).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
            assert!(worst < 0.002, "{encoding:?}: GPU and CPU diverged by {worst}");
        }
    }

    #[test]
    fn a_recipe_from_before_the_menu_reads_its_logc_switch_as_logc3() {
        let old: Recipe = serde_json::from_str(r#"{"use_logc": true}"#).unwrap();
        assert_eq!(old.encoding(), crate::LutEncoding::LogC3);
        let plain: Recipe = serde_json::from_str("{}").unwrap();
        assert_eq!(plain.encoding(), crate::LutEncoding::Display);
        let chosen: Recipe = serde_json::from_str(r#"{"use_logc": true, "lut_encoding": "cineon"}"#).unwrap();
        assert_eq!(chosen.encoding(), crate::LutEncoding::Cineon);
    }

    /// The render leaves in the chosen format: the same signal, encoded by that format's curve.
    #[test]
    fn the_render_leaves_in_the_chosen_format() {
        let mut recipe = rapid_recipe();
        recipe.lut_encoding = crate::LutEncoding::Linear;
        let linear = develop_px(&recipe, [0.18, 0.18, 0.18]);
        for encoding in [crate::LutEncoding::LogC3, crate::LutEncoding::Cineon] {
            recipe.lut_encoding = encoding;
            let out = develop_px(&recipe, [0.18, 0.18, 0.18]);
            for c in 0..3 {
                assert!(
                    (out[c] - encoding.encode(linear[c])).abs() < 1e-4,
                    "{encoding:?} channel {c}: {} vs {}",
                    out[c],
                    encoding.encode(linear[c])
                );
            }
        }
    }

    #[test]
    fn tone_curves_apply_in_non_display_encodings() {
        let mut flat = rapid_recipe();
        flat.lut_encoding = crate::LutEncoding::LogC3;
        let out_flat = develop_px(&flat, [0.18, 0.18, 0.18]);

        let mut curved = rapid_recipe();
        curved.lut_encoding = crate::LutEncoding::LogC3;
        curved.curve_luma = vec![[0.0, 0.0], [0.5, 0.8], [1.0, 1.0]];
        let out_curved = develop_px(&curved, [0.18, 0.18, 0.18]);

        assert!(
            (out_curved[0] - out_flat[0]).abs() > 0.05,
            "Tone curve must affect output when encoding is LogC3: flat {} vs curved {}",
            out_flat[0],
            out_curved[0]
        );
    }

    /// An identity `.cube` in a scratch folder, to put in a stack.
    fn identity_lut_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("reveal-lut-test-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut cube = String::from("LUT_3D_SIZE 2\n");
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    cube.push_str(&format!("{r}.0 {g}.0 {b}.0\n"));
                }
            }
        }
        std::fs::write(dir.join("identity.cube"), cube).unwrap();
        dir
    }

    /// Only the primaries are bookkeeping: a Linear stack that does nothing changes nothing.
    /// In any other format the curve is not undone afterwards — the stack's output is taken as
    /// it is, so the user's own conversion LUT is what brings the signal back.
    #[test]
    fn the_pre_lut_runs_in_the_chosen_format_and_undoes_nothing() {
        let dir = identity_lut_dir("pre");
        let input = ImageBuf::from_data(2, 1, vec![0.05, 0.1, 0.2, 0.4, 0.3, 0.1]);
        let render = |encoding: crate::LutEncoding, with_lut: bool| {
            let mut recipe = rapid_recipe();
            recipe.lut_encoding = encoding;
            if with_lut {
                recipe.rapid_pre_luts = vec![LutLayer { name: "identity".to_string(), opacity: 1.0 }];
            }
            develop_rapid_with(&input, &recipe, &dir, false).data
        };
        let plain = render(crate::LutEncoding::Linear, false);
        let through = render(crate::LutEncoding::Linear, true);
        for (a, b) in plain.iter().zip(through.iter()) {
            assert!((a - b).abs() < 2e-3, "a Linear identity stack moved the picture: {a} vs {b}");
        }
        // Display: the identity LUT returns the sRGB-encoded values, which are read as linear
        // — brighter than the same picture without the stack.
        let display_plain = render(crate::LutEncoding::Display, false);
        let display_through = render(crate::LutEncoding::Display, true);
        assert!(
            display_through.iter().sum::<f32>() > display_plain.iter().sum::<f32>() * 1.05,
            "the display curve must not be undone after the stack"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    /// The format is used once. A Pre-Lut stack eats it on the way in, and what the stack
    /// returns leaves as it is: a LogC3 identity stack gives the picture encoded once, not twice.
    #[test]
    fn a_pre_lut_stack_uses_the_format_and_the_render_does_not_encode_it_again() {
        let dir = identity_lut_dir("once");
        let px = [0.18f32, 0.18, 0.18];
        let input = ImageBuf::from_data(1, 1, px.to_vec());
        let mut recipe = rapid_recipe();
        recipe.lut_encoding = crate::LutEncoding::LogC3;
        recipe.rapid_pre_luts = vec![LutLayer { name: "identity".to_string(), opacity: 1.0 }];
        for use_gpu in [false, true] {
            if use_gpu && !crate::rapid_gpu::available() {
                continue;
            }
            let out = develop_rapid_with(&input, &recipe, &dir, use_gpu).data;
            let once = crate::LutEncoding::LogC3.encode(0.18);
            for c in 0..3 {
                assert!(
                    (out[c] - once).abs() < 0.01,
                    "gpu={use_gpu}: channel {c} left as {} instead of the stack's own output {once}",
                    out[c]
                );
            }
        }
        let _ = std::fs::remove_dir_all(dir);
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

        let layers = Layers::from_recipe(&recipe);
        let out = crate::rapid_gpu::run(
            &crate::rapid_gpu::Inputs {
                width: w,
                height: h,
                data: &input.data,
                blurred: &[],
                down_w: 0,
                down_h: 0,
                layers: &layers,
                zone_active: layers.zone_active(),
                has_color_wheels: false,
                output_encoding: recipe.encoding(),
                range: PhotoRange::REFERENCE,
                zone_range: PhotoRange::REFERENCE,
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
            let (s, m, h) = zone_weights(luma, luma, luma, 0.5);
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
        zoned.zone_shadows.exposure_ev = -1.0;
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
    fn test_zone_contrast_modulates_render_output() {
        // Local luma variation for a zone contrast pass to act on.
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
        zoned.zone_midtones.contrast = -0.3;
        let out_zoned = develop_rapid(&input, &zoned, Path::new(""));

        assert_ne!(
            out_zoned.data, out_base.data,
            "a zone-contrast-only recipe must actually change the render output"
        );
    }
    // ------------------------------------------------------------ tonal zones

    /// One pixel through the CPU reference path, as part of a photo whose own
    /// black is the calibration black (a grey at -9 stops) and whose white is
    /// a stop above the sensor's: the sliders measure the photo, and a lone
    /// pixel would be both its black and its white, where Highlights has
    /// nothing to bring down.
    fn develop_px(recipe: &Recipe, px: [f32; 3]) -> [f32; 3] {
        let dark = 2.0f32.powf(RANGE_BLACK_REF);
        let white = 2.0f32.powf(RANGE_WHITE_REF + 1.0);
        let mut data = px.to_vec();
        data.extend([dark; 3]);
        data.extend([white; 3]);
        let input = ImageBuf::from_data(3, 1, data);
        let out = develop_rapid_with(&input, recipe, Path::new(""), false);
        [out.data[0], out.data[1], out.data[2]]
    }

    /// Reach 50 is the partition the engine always had; more reach lets the masks overlap.
    #[test]
    fn the_mask_reach_sets_where_each_mask_falls_to_nothing() {
        let near = |a: f32, b: f32| (a - b).abs() < 1e-5;
        let at = |l: f32, reach: f32| zone_weights(l, l, l, reach);
        // The tone n = sqrt(L): 0.5 is L = 0.25.
        let (s, m, h) = at(0.25, 0.5);
        assert!(near(s, 0.0) && near(m, 1.0) && near(h, 0.0), "partition at the middle: {s} {m} {h}");
        for n in [0.0f32, 0.1, 0.3, 0.45, 0.5, 0.7, 0.95, 1.0] {
            let (s, m, h) = at(n * n, 0.5);
            assert!(near(s + m + h, 1.0), "the partition sums to 1 at n={n}: {}", s + m + h);
        }
        // Reach 75: the Shadows mask is gone at 75 % of the tones, the Highlights mask starts at 25 %.
        let (s, _, h) = at(0.75 * 0.75, 0.75);
        assert!(near(s, 0.0) && h > 0.0);
        let (s, _, h) = at(0.25 * 0.25, 0.75);
        assert!(s > 0.0 && near(h, 0.0));
        // Reach 100: each mask spans the whole range, with the eased (squared) tail.
        let (s, m, h) = at(0.0, 1.0);
        assert!(near(s, 1.0) && near(m, 0.0) && near(h, 0.0));
        let (s, m, h) = at(0.25, 1.0);
        assert!(near(s, 0.25) && near(m, 1.0) && near(h, 0.25), "{s} {m} {h}");
        let (s, _, h) = at(1.0, 1.0);
        assert!(near(s, 0.0) && near(h, 1.0));
    }

    /// What the screen shows of a grey ramp through the Shadows zone: the largest drop below an
    /// earlier, dimmer pixel (0 when brighter always means brighter).
    fn worst_inversion(recipe: &Recipe) -> f32 {
        let (mut peak, mut worst) = (0.0f32, 0.0f32);
        for i in 1..=300 {
            let l = i as f32 / 300.0;
            let shown = develop_px(recipe, [l, l, l])[1].min(1.0);
            peak = peak.max(shown);
            worst = worst.max(peak - shown);
        }
        worst
    }

    /// The reason the reach exists: with the default masks a strong lift in the shadows folds the
    /// picture back on itself and a smooth sky grows contour lines. From reach 75 the tail of the
    /// mask is eased and a lift of up to +3 EV keeps every tone in order.
    #[test]
    fn a_long_mask_reach_keeps_a_strong_lift_in_order() {
        let inversion = |reach: f32, ev: f32| {
            let mut recipe = rapid_recipe();
            recipe.zone_reach = reach;
            recipe.zone_shadows = ZoneAdjustments { exposure_ev: ev, ..ZoneAdjustments::default() };
            worst_inversion(&recipe)
        };
        assert!(inversion(50.0, 3.0) > 0.05, "the default masks are expected to fold at +3 EV");
        for reach in [75.0, 90.0, 100.0] {
            for ev in [1.0, 2.0, 3.0] {
                let worst = inversion(reach, ev);
                assert!(worst < 0.004, "reach {reach}, +{ev} EV: a brighter tone came out {worst} darker");
            }
        }
    }

    #[test]
    fn gpu_and_cpu_agree_on_strong_zones_and_any_mask_reach() {
        if !crate::rapid_gpu::available() {
            eprintln!("no GPU adapter — skipping GPU/CPU equivalence check");
            return;
        }
        let (w, h) = (48usize, 32usize);
        let mut data = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                let t = x as f32 / w as f32;
                data.extend([0.003 + t * t * 1.5, 0.003 + t * 0.9 + y as f32 / h as f32 * 0.1, 0.01 + (1.0 - t) * 0.4]);
            }
        }
        let input = ImageBuf::from_data(w as u32, h as u32, data);
        for reach in [50.0, 75.0, 100.0] {
            let mut recipe = rapid_recipe();
            recipe.zone_reach = reach;
            recipe.shadows_tint = [0.05, -0.02, 0.03];
            recipe.zone_shadows = ZoneAdjustments { exposure_ev: 3.0, shadows: 60.0, ..ZoneAdjustments::default() };
            recipe.zone_highlights = ZoneAdjustments { exposure_ev: -1.0, ..ZoneAdjustments::default() };
            let cpu = develop_rapid_with(&input, &recipe, Path::new(""), false);
            let gpu = develop_rapid_with(&input, &recipe, Path::new(""), true);
            let worst = cpu.data.iter().zip(gpu.data.iter()).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
            assert!(worst < 0.002, "reach={reach}: GPU and CPU diverged by {worst}");
        }
    }

    fn rapid_recipe() -> Recipe {
        let mut r = Recipe::default();
        r.engine = "rapid".to_string();
        r
    }

    /// A zone nobody touched must cost nothing: it is not even considered
    /// active, so no weights are computed and no pass is run for it.
    #[test]
    fn untouched_zones_are_not_run() {
        let mut recipe = rapid_recipe();
        assert_eq!(Layers::from_recipe(&recipe).zone_active(), [false; 3]);

        recipe.zone_midtones.contrast = 0.1;
        assert_eq!(Layers::from_recipe(&recipe).zone_active(), [false, true, false]);

        // A curve alone makes a zone active; so does a colour band.
        let mut recipe = rapid_recipe();
        recipe.zone_highlights.curve_luma = vec![[0.0, 0.0], [0.5, 0.4], [1.0, 1.0]];
        recipe.zone_shadows.hsl_sat = vec![0.0, 0.0, 12.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        assert_eq!(Layers::from_recipe(&recipe).zone_active(), [true, false, true]);

        // …and a zone left at its identity values (explicit zeros, an identity
        // curve) is as good as untouched.
        let mut recipe = rapid_recipe();
        recipe.zone_shadows.curve_luma = vec![[0.0, 0.0], [1.0, 1.0]];
        recipe.zone_shadows.hsl_hue = vec![0.0; 8];
        assert_eq!(Layers::from_recipe(&recipe).zone_active(), [false; 3]);
    }

    /// Every adjustment, set in one zone, leaves a pixel that has no weight in
    /// that zone exactly as it was. This is the property that makes a zone a
    /// mask rather than a second global.
    #[test]
    fn a_zone_never_touches_pixels_outside_it() {
        type Set = fn(&mut ZoneAdjustments);
        let fields: [(&str, Set); 15] = [
            ("exposure_ev", |z| z.exposure_ev = 1.0),
            ("contrast", |z| z.contrast = 0.6),
            ("brightness", |z| z.brightness = 40.0),
            ("temperature", |z| z.temperature = 40.0),
            ("tint", |z| z.tint = 40.0),
            ("saturation", |z| z.saturation = 0.6),
            ("vibrance", |z| z.vibrance = 50.0),
            ("whites", |z| z.whites = 50.0),
            ("highlights", |z| z.highlights = -60.0),
            ("midtones", |z| z.midtones = 40.0),
            ("shadows", |z| z.shadows = 60.0),
            ("blacks", |z| z.blacks = -60.0),
            ("hsl", |z| z.hsl_sat = vec![30.0; 8]),
            ("curve_luma", |z| z.curve_luma = vec![[0.0, 0.1], [0.5, 0.7], [1.0, 1.0]]),
            ("curve_r", |z| z.curve_r = vec![[0.0, 0.0], [0.5, 0.7], [1.0, 1.0]]),
        ];
        // (zone, a pixel with no weight in it). Pure black is all shadow, pure white
        // (and brighter) all highlight; a midtone has no weight at either.
        let outside: [(usize, [f32; 3]); 3] = [
            (0, [0.9, 0.7, 0.5]),  // shadows: a bright pixel
            (1, [0.0, 0.0, 0.0]),  // midtones: black
            (2, [0.01, 0.008, 0.006]), // highlights: a dark pixel
        ];
        for (zone, px) in outside {
            let (ws, wm, wh) = zone_weights(px[0], px[1], px[2], 0.5);
            assert_eq!([ws, wm, wh][zone], 0.0, "test pixel must be outside zone {zone}");
            let base = develop_px(&rapid_recipe(), px);
            for (name, set) in fields {
                let mut recipe = rapid_recipe();
                let z = match zone {
                    0 => &mut recipe.zone_shadows,
                    1 => &mut recipe.zone_midtones,
                    _ => &mut recipe.zone_highlights,
                };
                set(z);
                assert_eq!(develop_px(&recipe, px), base, "{name} in zone {zone} moved a pixel outside it");
            }
        }
    }

    /// And inside its zone, each adjustment does something. (Not every
    /// adjustment acts in every zone: a Highlights slider in the shadows zone
    /// has nothing to work on, as in any luminosity-range mask.)
    #[test]
    fn zone_adjustments_act_inside_their_zone() {
        type Set = fn(&mut ZoneAdjustments);
        // (zone, name, setter, a pixel the zone owns)
        let dark = [0.012, 0.008, 0.004];
        let mid = [0.16, 0.12, 0.08];
        let bright = [1.8, 1.4, 1.1];
        let cases: Vec<(usize, &str, Set, [f32; 3])> = vec![
            (0, "exposure_ev", |z| z.exposure_ev = 1.0, dark),
            (0, "contrast", |z| z.contrast = 0.8, dark),
            (0, "brightness", |z| z.brightness = 50.0, dark),
            (0, "temperature", |z| z.temperature = 50.0, dark),
            (0, "tint", |z| z.tint = 50.0, dark),
            (0, "saturation", |z| z.saturation = -0.8, dark),
            (0, "vibrance", |z| z.vibrance = 80.0, [0.03, 0.012, 0.006]),
            (0, "shadows", |z| z.shadows = 80.0, dark),
            (0, "blacks", |z| z.blacks = 80.0, dark),
            (0, "hsl", |z| z.hsl_sat = vec![-60.0; 8], [0.03, 0.012, 0.006]),
            (0, "curve_luma", |z| z.curve_luma = vec![[0.0, 0.2], [1.0, 1.0]], dark),
            (1, "exposure_ev", |z| z.exposure_ev = 1.0, mid),
            (1, "contrast", |z| z.contrast = 0.8, mid),
            (1, "brightness", |z| z.brightness = 50.0, mid),
            (1, "temperature", |z| z.temperature = 50.0, mid),
            (1, "saturation", |z| z.saturation = -0.8, mid),
            (1, "curve_g", |z| z.curve_g = vec![[0.0, 0.0], [0.5, 0.8], [1.0, 1.0]], mid),
            (2, "exposure_ev", |z| z.exposure_ev = -1.0, bright),
            (2, "highlights", |z| z.highlights = -80.0, bright),
            (2, "whites", |z| z.whites = 80.0, bright),
            (2, "saturation", |z| z.saturation = -0.8, bright),
            (2, "curve_b", |z| z.curve_b = vec![[0.0, 0.0], [0.5, 0.2], [1.0, 1.0]], bright),
        ];
        for (zone, name, set, px) in cases {
            let (ws, wm, wh) = zone_weights(px[0], px[1], px[2], 0.5);
            assert!([ws, wm, wh][zone] > 0.3, "test pixel should sit in zone {zone}, weights {ws:.2}/{wm:.2}/{wh:.2}");
            let base = develop_px(&rapid_recipe(), px);
            let mut recipe = rapid_recipe();
            let z = match zone {
                0 => &mut recipe.zone_shadows,
                1 => &mut recipe.zone_midtones,
                _ => &mut recipe.zone_highlights,
            };
            set(z);
            let out = develop_px(&recipe, px);
            let moved: f32 = out.iter().zip(base).map(|(a, b)| (a - b).abs()).sum();
            assert!(moved > 1e-4, "{name} in zone {zone} did nothing to a pixel it owns");
        }
    }

    /// The same slider means the same thing in a zone as globally: a zone that
    /// owns nearly the whole pixel and a global move of the same amount land
    /// almost in the same place (the zone is blended in by its weight, so it
    /// can fall a little short, never beyond). The old zone code had its own
    /// percent-scaled formulas and could not say this.
    #[test]
    fn a_zone_slider_has_the_global_slider_meaning() {
        let px = [0.0005, 0.0004, 0.0003];
        let (ws, ..) = zone_weights(px[0], px[1], px[2], 0.5);
        assert!(ws > 0.95, "the pixel should be nearly all shadow, weight {ws}");

        let base = develop_px(&rapid_recipe(), px);
        for (name, set_global, set_zone) in [
            ("exposure", (|r: &mut Recipe| r.exposure_ev = 5.0) as fn(&mut Recipe), (|z: &mut ZoneAdjustments| z.exposure_ev = 5.0) as fn(&mut ZoneAdjustments)),
        ] {
            let mut global = rapid_recipe();
            set_global(&mut global);
            let mut zoned = rapid_recipe();
            set_zone(&mut zoned.zone_shadows);
            let g = develop_px(&global, px);
            let z = develop_px(&zoned, px);
            for k in 0..3 {
                let (dg, dz) = (g[k] - base[k], z[k] - base[k]);
                assert!(dg.abs() > 1e-5, "{name}: the global move should be visible on channel {k}");
                assert!(dg * dz >= 0.0, "{name}: channel {k} moved the other way in the zone");
                assert!(dz.abs() <= dg.abs() * 1.001, "{name}: channel {k} went further in the zone ({dz}) than globally ({dg})");
                assert!(dz.abs() >= dg.abs() * 0.8, "{name}: channel {k} fell short in the zone ({dz}) of the global move ({dg})");
            }
        }
    }


    // ------------------------------------------------ the photo's own range

    fn adjust_with(f: impl Fn(&mut AdjustValues)) -> Adjust {
        let z = [0.0f32; 8];
        let mut v = AdjustValues {
            exposure_ev: 0.0, contrast: 0.0, saturation: 0.0, temperature: 0.0, tint: 0.0, whites: 0.0,
            highlights: 0.0, midtones: 0.0, shadows: 0.0, brightness: 0.0, blacks: 0.0, vibrance: 0.0,
            clarity: 0.0, structure: 0.0, dehaze: 0.0, hsl_hue: &z, hsl_sat: &z, hsl_lum: &z,
        };
        f(&mut v);
        Adjust::new(&v)
    }

    /// Stops a neutral grey of luminance `lum` moves through the tone stage
    /// when it sits in a flat neighbourhood of the same luminance.
    fn grey_shift(adj: &Adjust, range: &PhotoRange, lum: f32) -> f32 {
        let img = ImageBuf::from_data(16, 16, [lum; 3].repeat(256));
        let (b, dw, dh) = build_guidance(&img, 16, 16, adj, range);
        let guide = Guidance { blurred: &b, down_w: dw, down_h: dh };
        let o = adj.tone([lum; 3], 8, 8, &guide, range);
        (luma(o[0], o[1], o[2]).max(1e-9) / lum).log2()
    }

    fn log_ramp(lo: f32, hi: f32, n: usize) -> Vec<f32> {
        (0..n)
            .flat_map(|i| {
                let v = 2.0f32.powf(lo + (hi - lo) * i as f32 / (n - 1) as f32);
                [v, v, v]
            })
            .collect()
    }

    /// The range is where the photo's own darkest and brightest tones are, and
    /// it does not follow a stray pixel.
    #[test]
    fn the_photos_range_is_measured_from_the_frame() {
        let r = PhotoRange::measure(&log_ramp(-9.0, -1.0, 10_000));
        assert!((r.black + 9.0).abs() < 0.3, "black {}", r.black);
        assert!((r.white + 1.0).abs() < 0.3, "white {}", r.white);

        // One blown pixel in ten thousand does not move the white.
        let mut hot = log_ramp(-9.0, -1.0, 10_000);
        hot.extend([16.0; 3]);
        let h = PhotoRange::measure(&hot);
        assert!((h.white - r.white).abs() < 0.1, "a single hot pixel moved the white to {}", h.white);

        // A frame with nothing to measure leaves the calibration alone.
        assert_eq!(PhotoRange::measure(&vec![0.0; 3 * 5000]), PhotoRange::REFERENCE);
        assert_eq!(PhotoRange::measure(&vec![0.18; 3 * 5000]), PhotoRange::REFERENCE);
    }

    /// Blacks and Shadows act at the same place RELATIVE TO THE PHOTO's black,
    /// Highlights and Whites relative to its white: the same slider on a photo
    /// whose range sits lower or higher must do the same thing at the same
    /// distance from its own end.
    #[test]
    fn the_sliders_follow_the_photos_black_and_white() {
        let reference = PhotoRange { black: -9.0, white: 0.0 };
        let low_key = PhotoRange { black: -11.5, white: -3.0 };
        for (name, adj, from_black) in [
            ("blacks +50", adjust_with(|v| v.blacks = 50.0), true),
            ("blacks -50", adjust_with(|v| v.blacks = -50.0), true),
            ("shadows +50", adjust_with(|v| v.shadows = 50.0), true),
            ("shadows -50", adjust_with(|v| v.shadows = -50.0), true),
            ("highlights +50", adjust_with(|v| v.highlights = 50.0), false),
            ("highlights -50", adjust_with(|v| v.highlights = -50.0), false),
        ] {
            for distance in [1.0f32, 2.5, 4.0] {
                let stop = |r: &PhotoRange| if from_black { r.black + distance } else { r.white - distance.min(2.0) };
                let a = grey_shift(&adj, &reference, 2.0f32.powf(stop(&reference)));
                let b = grey_shift(&adj, &low_key, 2.0f32.powf(stop(&low_key)));
                assert!((a - b).abs() < 0.02, "{name} {distance} stops from the photo's end: {a:.3} vs {b:.3}");
            }
        }
        // And it is not nothing: Highlights +50 opens the top of a low-key photo.
        let opened = grey_shift(&adjust_with(|v| v.highlights = 50.0), &low_key, 2.0f32.powf(low_key.white));
        assert!(opened > 0.5, "Highlights +50 should open a low-key photo's own highlights, moved {opened}");
    }

    /// Whites opens or closes the top of the photo and leaves its shadows alone
    /// — it is no longer a second exposure slider.
    #[test]
    fn whites_acts_at_the_photos_white_only() {
        let range = PhotoRange { black: -9.0, white: -2.0 };
        let adj = adjust_with(|v| v.whites = 50.0);
        let full = adj.w_mult.log2();
        let at_white = grey_shift(&adj, &range, 2.0f32.powf(range.white));
        let deep = grey_shift(&adj, &range, 2.0f32.powf(range.white - 7.0));
        assert!((at_white - full).abs() < 0.03, "at the photo's white: {at_white} vs the full gain {full}");
        assert!(deep.abs() < 0.01, "seven stops below the white it should not move, moved {deep}");
        // Between the two it fades, monotonically.
        let mut last = deep - 1e-3;
        for d in [-6.0f32, -4.0, -2.0, -1.0, 0.0] {
            let v = grey_shift(&adj, &range, 2.0f32.powf(range.white + d));
            assert!(v >= last - 1e-4, "Whites is not monotone towards the white at {d}");
            last = v;
        }
    }

    /// The zones read the range the global layer leaves — same measurement,
    /// shifted by what the global layer did to the whole picture.
    #[test]
    fn the_zones_read_the_range_the_global_layer_leaves() {
        let range = PhotoRange { black: -9.0, white: -1.0 };
        let global = adjust_with(|v| v.exposure_ev = 1.0);
        let after = global.range_after(range);
        assert!((after.black + 8.0).abs() < 1e-4 && (after.white - 0.0).abs() < 1e-4);
        let whites = adjust_with(|v| v.whites = 50.0);
        assert!((whites.range_after(range).white - (range.white + whites.w_mult.log2())).abs() < 1e-5);
    }

    /// Clarity and Structure had a double /100: they did almost nothing, and the
    /// guidance map (gamma) was compared with linear luminance. Together: a flat
    /// patch must come out unchanged, and detail on it must be amplified by about
    /// what the slider says.
    #[test]
    fn clarity_and_structure_amplify_detail_and_leave_flat_areas_alone() {
        let range = PhotoRange::REFERENCE;
        for (name, adj) in [
            ("clarity +50", adjust_with(|v| v.clarity = 50.0)),
            ("clarity -40", adjust_with(|v| v.clarity = -40.0)),
            ("structure +50", adjust_with(|v| v.structure = 50.0)),
        ] {
            for lum in [0.05f32, 0.18, 0.5] {
                let flat = grey_shift(&adj, &range, lum);
                assert!(flat.abs() < 0.02, "{name} changed a flat patch of {lum} by {flat} stops");
            }
        }
        // A pixel 0.3 stop above its neighbourhood, over a mid-grey base.
        let adj = adjust_with(|v| v.clarity = 50.0);
        let base = 0.18f32;
        let img = ImageBuf::from_data(16, 16, [base; 3].repeat(256));
        let (b, dw, dh) = build_guidance(&img, 16, 16, &adj, &range);
        let guide = Guidance { blurred: &b, down_w: dw, down_h: dh };
        let up = base * 2.0f32.powf(0.3);
        let hi = luma_of_out(adj.tone([up; 3], 8, 8, &guide, &range));
        let lo = luma_of_out(adj.tone([base; 3], 8, 8, &guide, &range));
        let gain = (hi / lo).log2() / 0.3;
        assert!((gain - 1.5).abs() < 0.1, "clarity +50 should amplify detail about 1.5x, got {gain}");
    }

    fn luma_of_out(c: [f32; 3]) -> f32 {
        luma(c[0], c[1], c[2]).max(1e-9)
    }

    #[test]
    fn silvergrain_is_isotropic_and_emulates_film() {
        let (w, h) = (128u32, 128u32);
        let mut pixels = vec![0.35f32; (w * h * 3) as usize];
        super::apply_silvergrain(&mut pixels, w, h, 0.5, 0.5);

        // Check variance is non-zero
        let diffs: Vec<f32> = pixels.iter().map(|&p| p - 0.35).collect();
        let var: f32 = diffs.iter().map(|&d| d * d).sum::<f32>() / diffs.len() as f32;
        assert!(var > 1e-6, "grain must add visible texture");

        // Verify isotropy: mean horizontal neighbour difference should match
        // mean vertical neighbour difference within statistical tolerance (no directional lines/stripes).
        let mut h_diff_acc = 0.0f32;
        let mut v_diff_acc = 0.0f32;
        let mut count = 0.0f32;
        for y in 0..(h - 1) as usize {
            for x in 0..(w - 1) as usize {
                let p00 = pixels[(y * w as usize + x) * 3];
                let p10 = pixels[(y * w as usize + (x + 1)) * 3];
                let p01 = pixels[((y + 1) * w as usize + x) * 3];
                h_diff_acc += (p10 - p00).abs();
                v_diff_acc += (p01 - p00).abs();
                count += 1.0;
            }
        }
        let h_diff = h_diff_acc / count;
        let v_diff = v_diff_acc / count;
        let ratio = (h_diff / v_diff - 1.0).abs();
        assert!(
            ratio < 0.05,
            "grain must be isotropic: h_diff {h_diff} vs v_diff {v_diff} (ratio deviation: {ratio})"
        );
    }
}



