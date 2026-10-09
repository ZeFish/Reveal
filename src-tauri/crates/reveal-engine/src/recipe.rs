use serde::{Deserialize, Serialize};
use crate::encoding::LutEncoding;

/// The reference recipe stocks — same defaults the Swift app hardcoded.
pub const DEFAULT_FILM: &str = "kodak_gold_200";
pub const DEFAULT_PAPER: &str = "kodak_portra_endura";

pub fn default_engine() -> String {
    "spektra".into()
}

/// A develop recipe — the full user-facing parameter surface. This struct
/// IS the persisted `reveal:EngineSettings` schema (serde JSON), so field
/// names are the contract with the sidecars.
///
/// Scale conventions: 1.0 = the spektrafilm reference behavior, 0 = off.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Recipe {
    #[serde(default = "default_engine")]
    pub engine: String,

    pub film: String,
    pub paper: String,
    /// Manual exposure compensation, EV. Applies on top of auto-exposure
    /// when that is on (spektrafilm semantics).
    pub exposure_ev: f32,
    /// Center-weighted auto exposure (the Python reference's default).
    pub auto_exposure: bool,
    /// Enlarger print exposure, EV (0 = the auto-normalized print).
    pub print_exposure_ev: f32,
    /// Local contrast prepared for the film, 0 (off) to 1 (the full recipe): see `film_prep`.
    #[serde(default)]
    pub film_prep: f32,
    /// Enlarger filter shifts around the paper's calibrated neutral.
    pub y_shift: f32,
    pub m_shift: f32,
    pub film_format_mm: f32,
    /// Grain intensity: scales the physical particle area (0 = off).
    pub grain: f32,
    /// Halation + in-emulsion scatter strength (0 = off).
    pub halation: f32,
    /// Halation spatial scale (bigger = wider glow).
    pub halation_size: f32,
    /// Print-stage diffusion (Black Pro-Mist) strength; 0 = off.
    #[serde(default = "default_diffusion")]
    pub diffusion: f32,
    /// Scanner unsharp-mask amount scale.
    pub sharpen: f32,
    /// Scanner glare (stochastic; harness turns it off for determinism).
    pub glare: bool,
    /// Glare bloom strength, applied to both film and print stages.
    #[serde(default = "default_glare_percent")]
    pub glare_percent: f32,
    /// Glare bloom character: 0 = smooth, 1 = rough/textured.
    #[serde(default = "default_glare_roughness")]
    pub glare_roughness: f32,
    /// Glare bloom radius.
    #[serde(default = "default_glare_blur")]
    pub glare_blur: f32,
    /// B&W development time, minutes (push/pull processing). 0 = auto (the
    /// profile's floor-middle family entry). The pipeline snaps whatever
    /// value is given to the nearest entry in the film's own family, so any
    /// in-range value is safe — ignored entirely by colour profiles.
    #[serde(default)]
    pub development_time_min: f32,
    /// Global density-curve contrast adjustment (0 = neutral), applied to
    /// both film and print stages as `1.0 + density_gamma`.
    #[serde(default)]
    pub density_gamma: f32,

    /// Pre-flash: a small fogging exposure onto the print before the main
    /// exposure, lifting shadow density to compress contrast — a classic
    /// darkroom technique for a high-contrast negative. 0 = off
    /// (spektrafilm-rs's own default). Precomputed once at pipeline
    /// construction (`compute_preflash_raw`), not re-derived on a cache hit —
    /// same trap as the Y/M filters and development_time before it, so this
    /// must be part of `pipeline_for`'s rebuild key in spektra.rs.
    #[serde(default)]
    pub preflash_exposure: f32,
    #[serde(default)]
    pub preflash_y_shift: f32,
    #[serde(default)]
    pub preflash_m_shift: f32,

    /// DIR-couplers: inter-layer dye color interaction during development —
    /// real film chemistry, not a stylistic filter. spektrafilm-rs defaults
    /// this ON; `dir_couplers_active` is the opt-OUT, everything else here
    /// only matters while it's on.
    #[serde(default = "default_true")]
    pub dir_couplers_active: bool,
    #[serde(default = "default_dir_couplers_amount")]
    pub dir_couplers_amount: f32,
    #[serde(default = "default_dir_couplers_diffusion_size")]
    pub dir_couplers_diffusion_size: f32,
    #[serde(default = "default_dir_couplers_diffusion_tail")]
    pub dir_couplers_diffusion_tail: f32,
    #[serde(default = "default_dir_couplers_tail_weight")]
    pub dir_couplers_tail_weight: f32,

    // Tonal controls (Chantier 5)
    pub whites: f32,
    pub highlights: f32,
    pub midtones: f32,
    pub shadows: f32,
    pub rolloff: f32,

    // Rapid Engine digital controls
    pub contrast: f32,
    pub saturation: f32,
    pub temperature: f32,
    pub tint: f32,
    /// Old recipes only: the LogC switch that the encoding menu replaced. Read through
    /// `Recipe::encoding`, never written again.
    #[serde(default)]
    pub use_logc: bool,
    /// The tone, in percent, where the Shadows mask has fallen to nothing (and the Highlights
    /// mask begins): 50 splits the tones into three parts that never overlap, 100 stretches
    /// each mask across the whole range. See `rapid.rs::zone_weights`.
    #[serde(default = "default_zone_reach")]
    pub zone_reach: f32,
    /// The format the LUT stacks work in; see `encoding.rs`.
    #[serde(default)]
    pub lut_encoding: LutEncoding,
    #[serde(default = "default_agx_look")]
    pub agx_look: String,
    pub hsl_hue: Vec<f32>,
    pub hsl_sat: Vec<f32>,
    pub hsl_lum: Vec<f32>,
    pub shadows_tint: [f32; 3],
    pub midtones_tint: [f32; 3],
    pub highlights_tint: [f32; 3],

    /// The three tonal zones. Each is a luminosity mask carrying a full set of
    /// the same adjustments as the global ones, in the same units, applied on
    /// top of the global result (see `Layers` in rapid.rs). A zone left at its
    /// defaults costs nothing.
    #[serde(default)]
    pub zone_shadows: ZoneAdjustments,
    #[serde(default)]
    pub zone_midtones: ZoneAdjustments,
    #[serde(default)]
    pub zone_highlights: ZoneAdjustments,

    // Expose all RapidRaw sliders
    pub brightness: f32,
    pub blacks: f32,
    pub vibrance: f32,
    pub clarity: f32,
    pub dehaze: f32,
    pub structure: f32,
    pub vignette_amount: f32,
    #[serde(default = "default_vignette_midpoint")]
    pub vignette_midpoint: f32,
    #[serde(default = "default_vignette_roundness")]
    pub vignette_roundness: f32,
    #[serde(default = "default_vignette_feather")]
    pub vignette_feather: f32,
    pub grain_amount: f32,
    #[serde(default = "default_grain_roughness")]
    pub grain_roughness: f32,
    /// Strength of the highlight desaturation rolloff in AgX tonemapping
    /// (0 = blown highlights keep full saturation, 1 = fully neutral).
    #[serde(default = "default_highlight_desat")]
    pub highlight_desat: f32,

    /// Display-referred tone curves, as control points in 0..1 (x = input,
    /// y = output). `curve_luma` moves all three channels together; the per
    /// channel ones run after it. Two points at the corners = identity, and
    /// the renderer skips the stage entirely in that case, so the default
    /// costs nothing. Serde defaults keep every sidecar written before
    /// curves existed loading unchanged.
    #[serde(default = "default_curve")]
    pub curve_luma: Vec<[f32; 2]>,
    #[serde(default = "default_curve")]
    pub curve_r: Vec<[f32; 2]>,
    #[serde(default = "default_curve")]
    pub curve_g: Vec<[f32; 2]>,
    #[serde(default = "default_curve")]
    pub curve_b: Vec<[f32; 2]>,

    // User `.cube` LUTs applied to the raw scene-linear input in Rapid engine only,
    /// before exposure and tone controls — a creative pre-grade for digital RAW.
    /// Spektra ignores these. Stacked in order.
    #[serde(default)]
    pub rapid_pre_luts: Vec<LutLayer>,
    /// User `.cube` LUTs applied after Rapid tone mapping (display-encoded), as a
    /// finishing pass. Spektra ignores these. Stacked in order.
    #[serde(default)]
    pub rapid_post_luts: Vec<LutLayer>,

    // Crop & Orientation controls
    #[serde(default)]
    pub crop_x: f32,
    #[serde(default)]
    pub crop_y: f32,
    #[serde(default = "default_crop_dim")]
    pub crop_w: f32,
    #[serde(default = "default_crop_dim")]
    pub crop_h: f32,
    #[serde(default = "default_crop_aspect")]
    pub crop_aspect: String,
    #[serde(default)]
    pub crop_angle: f32,
    #[serde(default)]
    pub flip_h: bool,
    #[serde(default)]
    pub flip_v: bool,
    #[serde(default = "default_true")]
    pub apply_crop: bool,

    // Deprecated: kept for backward compatibility during migration.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pre_luts: Vec<LutLayer>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub post_luts: Vec<LutLayer>,
}

fn default_crop_dim() -> f32 {
    1.0
}

fn default_crop_aspect() -> String {
    "original".to_string()
}

fn default_zone_reach() -> f32 {
    50.0
}

fn default_agx_look() -> String {
    "base".to_string()
}

fn default_diffusion() -> f32 {
    0.75
}

// Match spektrafilm-rs's `GlareParams` defaults so leaving these sliders
// untouched reproduces the engine's existing look exactly.
fn default_glare_percent() -> f32 {
    0.03
}
fn default_glare_roughness() -> f32 {
    0.7
}
fn default_glare_blur() -> f32 {
    0.5
}

fn default_vignette_midpoint() -> f32 {
    0.5
}
fn default_vignette_roundness() -> f32 {
    0.5
}
fn default_vignette_feather() -> f32 {
    0.5
}
fn default_grain_roughness() -> f32 {
    0.12
}
fn default_curve() -> Vec<[f32; 2]> {
    crate::curves::IDENTITY.to_vec()
}

fn default_highlight_desat() -> f32 {
    0.4
}
fn default_true() -> bool {
    true
}
// spektrafilm-rs's own DirCouplersParams::default() values — kept identical
// so a fresh recipe (before anyone touches these sliders) renders exactly
// as it always has, DIR-couplers included, since Reveal never zeroed it out.
fn default_dir_couplers_amount() -> f32 {
    1.0
}
fn default_dir_couplers_diffusion_size() -> f32 {
    20.0
}
fn default_dir_couplers_diffusion_tail() -> f32 {
    200.0
}
fn default_dir_couplers_tail_weight() -> f32 {
    0.06
}

/// One layer of a LUT stack: a `.cube` file (by name, resolved against the
/// user's LUTs folder) blended in at `opacity` (0 = no effect, 1 = full).
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct LutLayer {
    pub name: String,
    #[serde(default = "default_lut_opacity")]
    pub opacity: f32,
}

fn default_lut_opacity() -> f32 {
    1.0
}

/// A tonal zone (shadows, midtones or highlights): a luminosity mask carrying
/// the same adjustments as the global ones — same fields, same units. Applied on
/// top of the global result and blended by `zone_weights`. Image-level stages
/// (input encoding, AgX look, vignette, grain, LUT stacks) stay global.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Default)]
pub struct ZoneAdjustments {
    #[serde(default)]
    pub exposure_ev: f32,
    #[serde(default)]
    pub contrast: f32,
    #[serde(default)]
    pub brightness: f32,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub tint: f32,
    #[serde(default)]
    pub saturation: f32,
    #[serde(default)]
    pub vibrance: f32,
    #[serde(default)]
    pub whites: f32,
    #[serde(default)]
    pub highlights: f32,
    #[serde(default)]
    pub midtones: f32,
    #[serde(default)]
    pub shadows: f32,
    #[serde(default)]
    pub blacks: f32,
    #[serde(default)]
    pub clarity: f32,
    #[serde(default)]
    pub structure: f32,
    #[serde(default)]
    pub dehaze: f32,
    #[serde(default)]
    pub hsl_hue: Vec<f32>,
    #[serde(default)]
    pub hsl_sat: Vec<f32>,
    #[serde(default)]
    pub hsl_lum: Vec<f32>,
    #[serde(default)]
    pub curve_luma: Vec<[f32; 2]>,
    #[serde(default)]
    pub curve_r: Vec<[f32; 2]>,
    #[serde(default)]
    pub curve_g: Vec<[f32; 2]>,
    #[serde(default)]
    pub curve_b: Vec<[f32; 2]>,
}

impl ZoneAdjustments {
    pub fn is_active(&self) -> bool {
        self.exposure_ev != 0.0
            || self.contrast != 0.0
            || self.brightness != 0.0
            || self.temperature != 0.0
            || self.tint != 0.0
            || self.saturation != 0.0
            || self.vibrance != 0.0
            || self.whites != 0.0
            || self.highlights != 0.0
            || self.midtones != 0.0
            || self.shadows != 0.0
            || self.blacks != 0.0
            || self.clarity != 0.0
            || self.structure != 0.0
            || self.dehaze != 0.0
            || self.hsl_hue.iter().any(|&v| v != 0.0)
            || self.hsl_sat.iter().any(|&v| v != 0.0)
            || self.hsl_lum.iter().any(|&v| v != 0.0)
            || !self.curve_luma.is_empty()
            || !self.curve_r.is_empty()
            || !self.curve_g.is_empty()
            || !self.curve_b.is_empty()
    }
}

impl Recipe {
    /// The format the LUT stacks work in. A recipe saved before the menu carries only the
    /// LogC switch: it reads as LogC3.
    pub fn encoding(&self) -> LutEncoding {
        match (self.lut_encoding, self.use_logc) {
            (LutEncoding::Display, true) => LutEncoding::LogC3,
            (chosen, _) => chosen,
        }
    }
}

impl Default for Recipe {
    fn default() -> Self {
        Self {
            engine: default_engine(),
            film: DEFAULT_FILM.into(),
            paper: DEFAULT_PAPER.into(),
            exposure_ev: 0.0,
            auto_exposure: true,
            print_exposure_ev: 0.0,
            film_prep: 0.0,
            y_shift: 0.0,
            m_shift: 0.0,
            film_format_mm: 35.0,
            grain: 1.0,
            halation: 1.0,
            halation_size: 1.0,
            diffusion: 0.75,
            sharpen: 1.0,
            glare: true,
            glare_percent: default_glare_percent(),
            glare_roughness: default_glare_roughness(),
            glare_blur: default_glare_blur(),
            development_time_min: 0.0,
            density_gamma: 0.0,
            preflash_exposure: 0.0,
            preflash_y_shift: 0.0,
            preflash_m_shift: 0.0,
            dir_couplers_active: true,
            dir_couplers_amount: default_dir_couplers_amount(),
            dir_couplers_diffusion_size: default_dir_couplers_diffusion_size(),
            dir_couplers_diffusion_tail: default_dir_couplers_diffusion_tail(),
            dir_couplers_tail_weight: default_dir_couplers_tail_weight(),
            whites: 0.0,
            highlights: 0.0,
            midtones: 0.0,
            shadows: 0.0,
            rolloff: 0.0,
            contrast: 0.0,
            saturation: 0.0,
            temperature: 0.0,
            tint: 0.0,
            use_logc: false,
            lut_encoding: LutEncoding::Display,
            zone_reach: 50.0,
            agx_look: "base".to_string(),
            hsl_hue: vec![0.0; 8],
            hsl_sat: vec![0.0; 8],
            hsl_lum: vec![0.0; 8],
            shadows_tint: [0.0, 0.0, 0.0],
            midtones_tint: [0.0, 0.0, 0.0],
            highlights_tint: [0.0, 0.0, 0.0],
            zone_shadows: ZoneAdjustments::default(),
            zone_midtones: ZoneAdjustments::default(),
            zone_highlights: ZoneAdjustments::default(),
            brightness: 0.0,
            blacks: 0.0,
            vibrance: 0.0,
            clarity: 0.0,
            dehaze: 0.0,
            structure: 0.0,
            vignette_amount: 0.0,
            vignette_midpoint: 0.5,
            vignette_roundness: 0.5,
            vignette_feather: 0.5,
            grain_amount: 0.0,
            grain_roughness: default_grain_roughness(),
            highlight_desat: default_highlight_desat(),
            curve_luma: default_curve(),
            curve_r: default_curve(),
            curve_g: default_curve(),
            curve_b: default_curve(),
            rapid_pre_luts: Vec::new(),
            rapid_post_luts: Vec::new(),
            crop_x: 0.0,
            crop_y: 0.0,
            crop_w: 1.0,
            crop_h: 1.0,
            crop_aspect: "original".to_string(),
            crop_angle: 0.0,
            flip_h: false,
            flip_v: false,
            apply_crop: true,
            pre_luts: Vec::new(),
            post_luts: Vec::new(),
        }
    }
}

/// A film or paper stock available to the recipe pickers.
#[derive(Clone, Debug, Serialize)]
pub struct ProfileEntry {
    pub name: String,
    pub label: String,
    /// "filming" or "printing" (profile `info.stage`).
    pub stage: String,
    /// "positive" or "negative" (profile `info.film_type`).
    pub film_type: String,
    /// True for a "bw" `info.channel_model` profile.
    pub is_bw: bool,
    /// True when the profile holds a family of density curves, one per
    /// development time (Double-X, print film 2302): the only stocks for which
    /// spektrafilm-rs's `resolve_for_render` has anything to select, so the only
    /// ones the "Duration" slider does anything for. A bw profile with a single
    /// curve (Tri-X) and every colour profile ignore it.
    pub has_development_times: bool,
}
