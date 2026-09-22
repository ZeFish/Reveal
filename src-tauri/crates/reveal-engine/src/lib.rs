//! Film-look engine: spektrafilm-rs orchestration.
//!
//! The only crate that touches the spektrafilm API — an upstream upgrade is
//! this crate's churn alone. The pipeline runs the FULL spectral simulation
//! per image on GPU (wgpu); there is no baked-LUT indirection anymore.
//!
//! Render economics (why the caches exist):
//!   decode (libraw, full res)      ~2.5 s   → cached per path
//!   downscale to preview           ~50 ms   → cached per (path, max_px)
//!   pipeline template               ~14 ms   → cached per (stocks, Y/M, EV)
//!   `with_params` + GPU process    ~100-200 ms @ 2 MP → paid per slider move
//!
//! So a slider drag costs only the last line; changing photo or stocks pays
//! its own cache miss and nothing else.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use reveal_decode::{DecoderRegistry, Primaries, RawDecoder};
use serde::{Deserialize, Serialize};
use spektrafilm_core::params::RuntimeParams;
use spektrafilm_core::profile;
use spektrafilm_gpu::ComputeBackend;
use spektrafilm_math::image::ImageBuf;

pub mod curves;
pub mod traits;
pub use traits::{EngineInfo, EngineRegistry, RenderEngine};

mod lut;
pub use lut::Cube;
pub mod rapid;
pub mod rapid_gpu;
pub mod spektra;
pub use rapid::develop_rapid;
pub use rapid::RapidEngine;
pub use spektra::SpektraEngine;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The reference recipe stocks — same defaults the Swift app hardcoded.
pub const DEFAULT_FILM: &str = "kodak_gold_200";
pub const DEFAULT_PAPER: &str = "kodak_portra_endura";

fn default_engine() -> String {
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
    /// Use LogC encoding instead of AgX tone mapping for cinematic look.
    #[serde(default)]
    pub use_logc: bool,
    #[serde(default = "default_agx_look")]
    pub agx_look: String,
    pub hsl_hue: Vec<f32>,
    pub hsl_sat: Vec<f32>,
    pub hsl_lum: Vec<f32>,
    pub shadows_tint: [f32; 3],
    pub midtones_tint: [f32; 3],
    pub highlights_tint: [f32; 3],

    /// Mask-free "soft zone" tone shaping: luminance-weighted local
    /// exposure/contrast/saturation per tonal zone (see `zone_weights` in
    /// rapid.rs). 0 = off. Distinct from `shadows`/`highlights`/`midtones`
    /// above, which are the Lightroom-style Basic-panel tone-recovery
    /// sliders — a different algorithm entirely.
    pub zone_shadows_exposure: f32,
    pub zone_shadows_contrast: f32,
    pub zone_shadows_saturation: f32,
    pub zone_midtones_exposure: f32,
    pub zone_midtones_contrast: f32,
    pub zone_midtones_saturation: f32,
    pub zone_highlights_exposure: f32,
    pub zone_highlights_contrast: f32,
    pub zone_highlights_saturation: f32,

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

    /// User `.cube` LUTs applied to the raw scene-linear input in Rapid engine only,
    /// before exposure and tone controls — a creative pre-grade for digital RAW.
    /// Spektra ignores these. Stacked in order.
    #[serde(default)]
    pub rapid_pre_luts: Vec<LutLayer>,
    /// User `.cube` LUTs applied after Rapid tone mapping (display-encoded), as a
    /// finishing pass. Spektra ignores these. Stacked in order.
    #[serde(default)]
    pub rapid_post_luts: Vec<LutLayer>,

    // Deprecated: kept for backward compatibility during migration.
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pre_luts: Vec<LutLayer>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub post_luts: Vec<LutLayer>,
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

impl Default for Recipe {
    fn default() -> Self {
        Self {
            engine: default_engine(),
            film: DEFAULT_FILM.into(),
            paper: DEFAULT_PAPER.into(),
            exposure_ev: 0.0,
            auto_exposure: true,
            print_exposure_ev: 0.0,
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
            agx_look: "base".to_string(),
            hsl_hue: vec![0.0; 8],
            hsl_sat: vec![0.0; 8],
            hsl_lum: vec![0.0; 8],
            shadows_tint: [0.0, 0.0, 0.0],
            midtones_tint: [0.0, 0.0, 0.0],
            highlights_tint: [0.0, 0.0, 0.0],
            zone_shadows_exposure: 0.0,
            zone_shadows_contrast: 0.0,
            zone_shadows_saturation: 0.0,
            zone_midtones_exposure: 0.0,
            zone_midtones_contrast: 0.0,
            zone_midtones_saturation: 0.0,
            zone_highlights_exposure: 0.0,
            zone_highlights_contrast: 0.0,
            zone_highlights_saturation: 0.0,
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
    /// True for a "bw" `info.channel_model` profile — the only kind
    /// spektrafilm-rs's `resolve_for_render` actually varies by development
    /// time (it collapses a family of push/pull density curves to the
    /// selected one); every color profile ignores development_time
    /// entirely, so the "Durée" slider is a no-op there by design.
    pub is_bw: bool,
}

pub struct RenderOutput {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Pipeline time only, milliseconds.
    pub render_ms: u128,
    /// RAW decode time (0 on decode-cache hit), milliseconds.
    pub decode_ms: u128,
}

pub struct RenderRgbaOutput {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub render_ms: u128,
    pub decode_ms: u128,
}

/// The engine: one GPU backend for the process lifetime, plus the caches
/// described in the module header. All methods take `&self`; internal
/// mutability is Mutex'd, so renders serialize (one GPU, one photo at a
/// time — latest-wins scheduling lives in the caller).
pub struct Engine {
    data_dir: PathBuf,
    /// Where the user's own `.cube` LUTs live (scanned for `list_luts`,
    /// distinct from spektrafilm-rs's internal `data_dir/luts` — that one is
    /// spectral-upsampling tables, not creative grading LUTs).
    luts_dir: PathBuf,
    backend: Box<dyn ComputeBackend>,
    decoder: DecoderRegistry,
    /// Decoded photo, already in ProPhoto working space. Keyed by (path, fast)
    /// so a half-res preview decode and a full-res export decode don't evict
    /// each other's meaning — `fast` previews and full exports cache apart.
    decoded: Mutex<Option<((PathBuf, bool), Arc<ImageBuf>)>>,
    /// Downscaled pipeline input for previews.
    preview_input: Mutex<Option<((PathBuf, u32), Arc<ImageBuf>)>>,
    /// Plugin engine registry.
    registry: EngineRegistry,
}

impl Engine {
    /// `data_dir` is spektrafilm-rs's data directory (profiles/, luts/,
    /// filters/) — vendored at src-tauri/data and bundled as a resource.
    /// `luts_dir` is the user's own, editable folder of `.cube` files (lives
    /// in the app's Application Support, created on first use if missing).
    pub fn new(data_dir: impl Into<PathBuf>, luts_dir: impl Into<PathBuf>) -> Result<Self> {
        let data_dir = data_dir.into();
        anyhow::ensure!(
            data_dir.join("profiles").is_dir(),
            "spektrafilm data dir not found at {}",
            data_dir.display()
        );
        let luts_dir = luts_dir.into();
        std::fs::create_dir_all(&luts_dir)
            .with_context(|| format!("creating LUTs dir {}", luts_dir.display()))?;

        let backend = spektrafilm_gpu::select_backend();
        let mut registry = EngineRegistry::new();
        registry.register(Arc::new(spektra::SpektraEngine::new(
            data_dir.clone(),
            spektrafilm_gpu::select_backend(),
        )));
        registry.register(Arc::new(rapid::RapidEngine));

        Ok(Self {
            data_dir,
            luts_dir,
            backend,
            decoder: DecoderRegistry,
            decoded: Mutex::new(None),
            preview_input: Mutex::new(None),
            registry,
        })
    }

    pub fn backend_name(&self) -> &str {
        self.backend.name()
    }

    pub fn luts_dir(&self) -> &Path {
        &self.luts_dir
    }

    /// `.cube` files available in the user's LUTs folder and any subfolders,
    /// by relative path name (no extension), sorted. Missing folder reads as empty.
    pub fn list_luts(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        if self.luts_dir.exists() {
            Self::scan_luts_dir(&self.luts_dir, &self.luts_dir, &mut names)?;
        }
        names.sort();
        Ok(names)
    }

    fn scan_luts_dir(base_dir: &Path, current_dir: &Path, names: &mut Vec<String>) -> Result<()> {
        let entries = match std::fs::read_dir(current_dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e).with_context(|| format!("reading {}", current_dir.display())),
        };

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                Self::scan_luts_dir(base_dir, &path, names)?;
            } else if path.is_file() {
                let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                if file_name.starts_with('.') {
                    continue;
                }
                if path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map_or(false, |ext| ext.eq_ignore_ascii_case("cube"))
                {
                    if let Ok(rel_path) = path.strip_prefix(base_dir) {
                        let mut rel_str = rel_path.to_string_lossy().to_string();
                        if rel_str.to_lowercase().ends_with(".cube") {
                            rel_str.truncate(rel_str.len() - 5);
                        }
                        names.push(rel_str);
                    }
                }
            }
        }
        Ok(())
    }

    /// The stocks available in `data_dir`, labeled, split by stage.
    pub fn list_profiles(&self) -> Result<Vec<ProfileEntry>> {
        let dir = self.data_dir.join("profiles");
        let mut entries = Vec::new();
        for e in std::fs::read_dir(&dir).with_context(|| format!("reading {}", dir.display()))? {
            let e = e?;
            let file = e.file_name().to_string_lossy().to_string();
            let Some(name) = file.strip_suffix(".json").map(str::to_string) else {
                continue;
            };
            match profile::load_profile_by_name(&self.data_dir, &name) {
                Ok(p) => entries.push(ProfileEntry {
                    label: p.info.name.clone().unwrap_or_else(|| name.clone()),
                    stage: p.info.stage.clone(),
                    film_type: p.info.film_type.clone(),
                    is_bw: p.is_bw(),
                    name,
                }),
                Err(_) => continue,
            }
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }

    pub fn list_engines(&self) -> Vec<EngineInfo> {
        self.registry.list()
    }

    /// Migrate deprecated global pre/post_luts to rapid_pre/post_luts if engine is "rapid".
    fn migrate_luts_if_needed(recipe: &mut Recipe) {
        if recipe.engine != "rapid" {
            return;
        }

        if !recipe.pre_luts.is_empty() {
            if recipe.rapid_pre_luts.is_empty() {
                recipe.rapid_pre_luts = std::mem::take(&mut recipe.pre_luts);
            } else {
                recipe.rapid_pre_luts.append(&mut recipe.pre_luts);
            }
        }

        if !recipe.post_luts.is_empty() {
            if recipe.rapid_post_luts.is_empty() {
                recipe.rapid_post_luts = std::mem::take(&mut recipe.post_luts);
            } else {
                recipe.rapid_post_luts.append(&mut recipe.post_luts);
            }
        }
    }

    /// Decode + engine pipeline. Shared by `develop_jpeg`, `develop_rgba8`, and
    /// `develop_rgb8`. Rapid now owns its LUT application internally so ordering stays
    /// engine-specific.
    fn develop(&self, path: &Path, recipe: &Recipe, max_px: u32) -> Result<(ImageBuf, u128, u128)> {
        let (input, decode_ms) = self.pipeline_input(path, max_px)?;

        let t = std::time::Instant::now();

        let mut recipe = recipe.clone();
        Self::migrate_luts_if_needed(&mut recipe);

        // Dispatch to selected RenderEngine trait implementation (Spektra, Rapid, etc.)
        let render_engine = self
            .registry
            .get(&recipe.engine)
            .context("resolving render engine from registry")?;

        let result = render_engine.render(&input, &recipe, &self.luts_dir)?;

        let render_ms = t.elapsed().as_millis();

        Ok((result, decode_ms, render_ms))
    }

    /// Decode + film pipeline + JPEG. `max_px` bounds the long edge
    /// (0 = full resolution).
    pub fn develop_jpeg(&self, path: &Path, recipe: &Recipe, max_px: u32) -> Result<RenderOutput> {
        let (result, decode_ms, render_ms) = self.develop(path, recipe, max_px)?;
        let rgb8 = quantize_rgb8(&result);
        let jpeg = encode_jpeg(&rgb8, result.width, result.height, 92)?;
        Ok(RenderOutput {
            jpeg,
            width: result.width,
            height: result.height,
            render_ms,
            decode_ms,
        })
    }

    /// Decode + film/rapid pipeline + RGBA8 uncompressed buffer for high-speed canvas display.
    pub fn develop_rgba8(
        &self,
        path: &Path,
        recipe: &Recipe,
        max_px: u32,
    ) -> Result<RenderRgbaOutput> {
        let (result, decode_ms, render_ms) = self.develop(path, recipe, max_px)?;
        let rgba = quantize_rgba8(&result);
        Ok(RenderRgbaOutput {
            rgba,
            width: result.width,
            height: result.height,
            render_ms,
            decode_ms,
        })
    }

    /// Same, returning the quantized RGB8 buffer (verification harness).
    pub fn develop_rgb8(
        &self,
        path: &Path,
        recipe: &Recipe,
        max_px: u32,
    ) -> Result<(Vec<u8>, u32, u32, u128, u128)> {
        let (result, decode_ms, render_ms) = self.develop(path, recipe, max_px)?;
        Ok((
            quantize_rgb8(&result),
            result.width,
            result.height,
            decode_ms,
            render_ms,
        ))
    }

    /// Full-resolution export: develop, resize to `long_edge` (0 = native),
    /// optional white print border, JPEG q95. Returns (jpeg, w, h).
    pub fn export_jpeg(
        &self,
        path: &Path,
        recipe: &Recipe,
        long_edge: u32,
        border_frac: f32,
    ) -> Result<(Vec<u8>, u32, u32)> {
        let (rgb8, w, h, _, _) = self.develop_rgb8(path, recipe, 0)?;
        let mut img: image::RgbImage =
            image::ImageBuffer::from_raw(w, h, rgb8).context("export buffer")?;

        if long_edge > 0 && w.max(h) > long_edge {
            let scale = long_edge as f64 / w.max(h) as f64;
            let (nw, nh) = (
                ((w as f64 * scale).round() as u32).max(1),
                ((h as f64 * scale).round() as u32).max(1),
            );
            img = image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Lanczos3);
        }

        if border_frac > 0.0 {
            img = paper_border(&img, border_frac);
        }

        let (w, h) = img.dimensions();
        let jpeg = encode_jpeg(img.as_raw(), w, h, 95)?;
        Ok((jpeg, w, h))
    }

    /// The (possibly downscaled) ProPhoto pipeline input for a photo,
    /// through both caches. Returns (input, decode_ms — 0 on cache hit).
    fn pipeline_input(&self, path: &Path, max_px: u32) -> Result<(Arc<ImageBuf>, u128)> {
        // Previews (max_px > 0) take the fast half-res decode; the export path
        // (max_px == 0) takes the full-quality decode.
        let fast = max_px != 0;
        let (full, decode_ms) = {
            let mut guard = self.decoded.lock().unwrap();
            match guard.as_ref() {
                Some(((p, f), img)) if p == path && *f == fast => (img.clone(), 0),
                _ => {
                    let t = std::time::Instant::now();
                    let linear = self
                        .decoder
                        .decode_linear(path, fast)
                        .with_context(|| format!("decoding {}", path.display()))?;
                    let data = to_prophoto(linear.data, linear.primaries);
                    let img = Arc::new(ImageBuf::from_data(linear.width, linear.height, data));
                    let ms = t.elapsed().as_millis();
                    *guard = Some(((path.to_path_buf(), fast), img.clone()));
                    // A new decode invalidates the preview-input cache too.
                    *self.preview_input.lock().unwrap() = None;
                    (img, ms)
                }
            }
        };

        if max_px == 0 || full.width.max(full.height) <= max_px {
            return Ok((full, decode_ms));
        }

        let key = (path.to_path_buf(), max_px);
        let mut guard = self.preview_input.lock().unwrap();
        if let Some((k, img)) = guard.as_ref() {
            if *k == key {
                return Ok((img.clone(), decode_ms));
            }
        }
        let small = Arc::new(downscale(&full, max_px));
        *guard = Some((key, small.clone()));
        Ok((small, decode_ms))
    }
}

/// Recipe → spektrafilm RuntimeParams. Upstream defaults everywhere else —
/// notably `print_exposure_compensation` + `normalize_print_exposure` stay
/// ON (the image-adaptive print normalization the Python reference has).
fn runtime_params(recipe: &Recipe) -> RuntimeParams {
    let mut p = RuntimeParams::default();

    // Input contract: scene-linear ProPhoto (the Python loader's space);
    // the decode boundary already converted (see `to_prophoto`).
    p.io.input_color_space = "ProPhoto RGB".to_string();
    p.io.input_cctf_decoding = false;

    p.camera.exposure_compensation_ev = recipe.exposure_ev;
    p.camera.auto_exposure = recipe.auto_exposure;
    p.camera.film_format_mm = recipe.film_format_mm;

    p.enlarger.print_exposure = 2f32.powf(recipe.print_exposure_ev);
    p.enlarger.y_filter_shift = recipe.y_shift;
    p.enlarger.m_filter_shift = recipe.m_shift;

    p.film_render.grain.active = recipe.grain > 0.0;
    if recipe.grain > 0.0 {
        p.film_render.grain.agx_particle_area_um2 *= f64::from(recipe.grain);
    }

    p.film_render.halation.active = recipe.halation > 0.0;
    if recipe.halation > 0.0 {
        p.film_render.halation.halation_amount *= f64::from(recipe.halation);
        p.film_render.halation.scatter_amount *= f64::from(recipe.halation);
        p.film_render.halation.halation_spatial_scale *= f64::from(recipe.halation_size);
        p.film_render.halation.scatter_spatial_scale *= f64::from(recipe.halation_size);
    }

    p.enlarger.diffusion_filter.active = recipe.diffusion > 0.0;
    if recipe.diffusion > 0.0 {
        p.enlarger.diffusion_filter.strength = recipe.diffusion;
    }

    // unsharp_mask = [sigma, amount]; the slider scales the amount.
    p.scanner.unsharp_mask[1] *= recipe.sharpen;

    p.film_render.glare.active = recipe.glare;
    p.film_render.glare.percent = recipe.glare_percent;
    p.film_render.glare.roughness = recipe.glare_roughness;
    p.film_render.glare.blur = recipe.glare_blur;
    p.print_render.glare.active = recipe.glare;
    p.print_render.glare.percent = recipe.glare_percent;
    p.print_render.glare.roughness = recipe.glare_roughness;
    p.print_render.glare.blur = recipe.glare_blur;

    if recipe.development_time_min > 0.0 {
        p.film_render.development_time = Some(f64::from(recipe.development_time_min));
    }

    p.film_render.density_curve_gamma = 1.0 + recipe.density_gamma;
    p.print_render.density_curve_gamma = 1.0 + recipe.density_gamma;

    p.enlarger.preflash_exposure = recipe.preflash_exposure;
    p.enlarger.preflash_y_filter_shift = recipe.preflash_y_shift;
    p.enlarger.preflash_m_filter_shift = recipe.preflash_m_shift;

    p.film_render.dir_couplers.active = recipe.dir_couplers_active;
    p.film_render.dir_couplers.amount = f64::from(recipe.dir_couplers_amount);
    p.film_render.dir_couplers.diffusion_size_um = f64::from(recipe.dir_couplers_diffusion_size);
    p.film_render.dir_couplers.diffusion_tail_um = f64::from(recipe.dir_couplers_diffusion_tail);
    p.film_render.dir_couplers.diffusion_tail_weight = f64::from(recipe.dir_couplers_tail_weight);

    p
}

/// Decoder primaries → linear ProPhoto RGB (the pipeline's working space,
/// same as the Python loader's output). Matrices computed with
/// colour-science `matrix_RGB_to_RGB(…, "ProPhoto RGB", CAT02)` — the exact
/// conversion `load_and_process_raw_file` applies. Inputs may hold
/// out-of-gamut negatives; the clamp happens here, AFTER the gamut widens,
/// where almost nothing real is negative anymore.
const SRGB_TO_PROPHOTO: [[f32; 3]; 3] = [
    [0.5288241004, 0.3340609866, 0.1373616909],
    [0.0975294148, 0.8790074094, 0.0233981175],
    [0.0163599018, 0.1066124933, 0.8772485185],
];
const ACES_TO_PROPHOTO: [[f32; 3]; 3] = [
    [1.2393803418, -0.1639678228, -0.0752333838],
    [0.0036113619, 1.0896136492, -0.0932657921],
    [-0.0020596793, -0.0022515883, 1.0045855773],
];

fn to_prophoto(mut data: Vec<f32>, primaries: Primaries) -> Vec<f32> {
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

/// Box-filter downscale in linear light so the long edge is `max_px`.
fn downscale(img: &ImageBuf, max_px: u32) -> ImageBuf {
    use rayon::prelude::*;
    let (w, h) = (img.width as usize, img.height as usize);
    let scale = max_px as f64 / img.width.max(img.height) as f64;
    let nw = ((img.width as f64 * scale).round() as usize).max(1);
    let nh = ((img.height as f64 * scale).round() as usize).max(1);

    let data: Vec<_> = (0..nh)
        .into_par_iter()
        .flat_map_iter(|oy| {
            let y0 = oy * h / nh;
            let y1 = (((oy + 1) * h) / nh).max(y0 + 1).min(h);
            let src = &img.data;
            (0..nw).flat_map(move |ox| {
                let x0 = ox * w / nw;
                let x1 = (((ox + 1) * w) / nw).max(x0 + 1).min(w);
                let mut acc = [0.0f64; 3];
                let mut n = 0.0f64;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let i = (y * w + x) * 3;
                        acc[0] += f64::from(src[i]);
                        acc[1] += f64::from(src[i + 1]);
                        acc[2] += f64::from(src[i + 2]);
                        n += 1.0;
                    }
                }
                [
                    spektrafilm_math::precision::from_f32((acc[0] / n) as f32),
                    spektrafilm_math::precision::from_f32((acc[1] / n) as f32),
                    spektrafilm_math::precision::from_f32((acc[2] / n) as f32),
                ]
            })
        })
        .collect();

    ImageBuf::from_data(nw as u32, nh as u32, data)
}

/// Quantize a pipeline result (display-encoded sRGB, [0,1]) to 8-bit —
/// `round_ties_even` stays numpy-identical with the reference tools.
fn quantize_rgb8(img: &ImageBuf) -> Vec<u8> {
    img.data
        .iter()
        .map(|&v| ((f64::from(v).clamp(0.0, 1.0) * 255.0).round_ties_even()) as u8)
        .collect()
}

/// Quantize a pipeline result to 8-bit RGBA for direct HTML5 Canvas rendering.
pub fn quantize_rgba8(img: &ImageBuf) -> Vec<u8> {
    let mut out = Vec::with_capacity((img.width * img.height * 4) as usize);
    for chunk in img.data.chunks_exact(3) {
        out.push((f64::from(chunk[0]).clamp(0.0, 1.0) * 255.0).round_ties_even() as u8);
        out.push((f64::from(chunk[1]).clamp(0.0, 1.0) * 255.0).round_ties_even() as u8);
        out.push((f64::from(chunk[2]).clamp(0.0, 1.0) * 255.0).round_ties_even() as u8);
        out.push(255);
    }
    out
}

/// Encode 8-bit interleaved RGB as JPEG.
fn encode_jpeg(rgb8: &[u8], width: u32, height: u32, quality: u8) -> Result<Vec<u8>> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(
        &mut std::io::Cursor::new(&mut out),
        quality,
    )
    .write_image(rgb8, width, height, image::ExtendedColorType::Rgb8)
    .context("encoding JPEG")?;
    Ok(out)
}

/// The print's paper border. Pure #FFFFFF reads as a flat digital void beside a
/// developed frame; a fine-art matte is a hair warm and carries a faint tooth.
/// Both knobs are deliberately near the edge of perception — nudge `PAPER_TINT`
/// warmer or `GRAIN_AMP` higher for a more textured stock, cooler/0 for clinical
/// white.
const PAPER_TINT: [u8; 3] = [253, 251, 248];
const GRAIN_AMP: i16 = 3;

/// Wrap `img` in the paper border, `border_frac` of the long edge on every side.
pub fn paper_border(img: &image::RgbImage, border_frac: f32) -> image::RgbImage {
    let (w, h) = img.dimensions();
    let b = ((w.max(h) as f32) * border_frac).round() as u32;
    let mut matte = paper_matte(w + 2 * b, h + 2 * b);
    image::imageops::overlay(&mut matte, img, b as i64, b as i64);
    matte
}

/// A sheet of paper: the warm-white base plus a stable per-pixel luminance
/// grain (same delta on all channels, so the tooth is neutral, never coloured).
/// The noise is a cheap position hash — deterministic, so re-exporting a frame
/// is byte-stable and two prints of the same size share the same grain field.
fn paper_matte(w: u32, h: u32) -> image::RgbImage {
    let span = 2 * GRAIN_AMP as u32 + 1;
    image::ImageBuffer::from_fn(w, h, |x, y| {
        let mut n = x
            .wrapping_mul(374_761_393)
            .wrapping_add(y.wrapping_mul(668_265_263));
        n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
        let noise = (n % span) as i16 - GRAIN_AMP;
        let px = |c: u8| (c as i16 + noise).clamp(0, 255) as u8;
        image::Rgb([px(PAPER_TINT[0]), px(PAPER_TINT[1]), px(PAPER_TINT[2])])
    })
}
