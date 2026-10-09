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
use reveal_decode::{DecoderRegistry, RawDecoder};
use spektrafilm_core::params::RuntimeParams;
use spektrafilm_core::profile;
use spektrafilm_gpu::ComputeBackend;
use spektrafilm_math::image::ImageBuf;

pub mod curves;
pub mod film_prep;
pub mod traits;
pub use traits::{EngineInfo, EngineRegistry, RenderEngine};

mod encoding;
pub use encoding::LutEncoding;
mod lut;
pub use lut::Cube;
pub mod rapid;
pub mod rapid_gpu;
pub mod spektra;
pub use rapid::develop_rapid;
pub use rapid::RapidEngine;
pub use spektra::SpektraEngine;

pub mod color;
pub use color::*;
pub mod gates;
pub(crate) use gates::Gates;
pub mod output;
pub use output::*;
pub mod parked;
pub use parked::*;
pub mod recipe;
pub use recipe::*;
pub mod transform;
pub use transform::*;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct Engine {
    data_dir: PathBuf,
    /// Where the user's own `.cube` LUTs live (scanned for `list_luts`,
    /// distinct from spektrafilm-rs's internal `data_dir/luts` — that one is
    /// spectral-upsampling tables, not creative grading LUTs).
    luts_dir: PathBuf,
    backend: Box<dyn ComputeBackend>,
    decoder: DecoderRegistry,
    /// Decoded photos, already in ProPhoto working space, most-recently-used
    /// first. Keyed by (path, fast) so a half-res preview decode and a
    /// full-res export decode don't evict each other's meaning.
    ///
    /// Multi-entry because a cull is mostly stepping back and forth: with a
    /// single slot, returning to the previous frame re-read it off the NAS
    /// and re-decoded it, ~4s for a frame that was in memory moments before.
    /// Bounded by bytes, see DECODE_CACHE_BUDGET_BYTES.
    decoded: Mutex<Vec<((PathBuf, bool), Arc<ImageBuf>)>>,
    /// Downscaled pipeline inputs for previews, same MRU-first ordering.
    preview_input: Mutex<Vec<((PathBuf, u32), Arc<ImageBuf>)>>,
    /// One gate per photo being decoded, so threads that want the SAME frame
    /// queue behind one decode instead of each running their own.
    decode_gates: Gates<(PathBuf, bool)>,
    /// One gate per downscaled preview input so concurrent renders don't downscale the same photo.
    preview_gates: Gates<(PathBuf, u32)>,
    /// Where a decoded frame may be parked so it survives a restart. Both
    /// caches above die with the process, so reopening the photo you were
    /// editing meant paying the full read and decode again — measured on a
    /// NAS-hosted library, ~3.1s of network then ~1.0s of CPU. Held on disk
    /// it comes back in the time a local read takes. Only ever one photo:
    /// the one open in Develop.
    working_dir: Mutex<Option<PathBuf>>,
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
            decoded: Mutex::new(Vec::new()),
            preview_input: Mutex::new(Vec::new()),
            decode_gates: Gates::default(),
            preview_gates: Gates::default(),
            working_dir: Mutex::new(None),
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
                    has_development_times: p.data.development_time.len() > 1,
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
        let t_pipe = std::time::Instant::now();
        let (input, decode_ms) = self.pipeline_input(path, max_px)?;
        let pipeline_ms = t_pipe.elapsed().as_millis();

        let t = std::time::Instant::now();

        let mut recipe = recipe.clone();
        Self::migrate_luts_if_needed(&mut recipe);

        // Dispatch to selected RenderEngine trait implementation (Spektra, Rapid, etc.)
        let render_engine = self
            .registry
            .get(&recipe.engine)
            .context("resolving render engine from registry")?;

        let mut result = render_engine.render(&input, &recipe, &self.luts_dir)?;

        if recipe.apply_crop {
            result = crop_and_flip(&result, &recipe);
        }

        let render_ms = t.elapsed().as_millis();

        eprintln!(
            "[perf] develop {}: pipeline_input {} ms (decode {} ms), render {} ms",
            path.display(),
            pipeline_ms,
            decode_ms,
            render_ms
        );

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
        let t0 = std::time::Instant::now();
        let (result, decode_ms, render_ms) = self.develop(path, recipe, max_px)?;
        let dev_ms = t0.elapsed().as_millis();
        let t_q = std::time::Instant::now();
        let rgba = quantize_rgba8(&result);
        let quantize_ms = t_q.elapsed().as_millis();
        eprintln!(
            "[perf] develop_rgba8 {}: total {} ms (develop {} ms, quantize {} ms)",
            path.display(),
            t0.elapsed().as_millis(),
            dev_ms,
            quantize_ms
        );
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
        let t0 = std::time::Instant::now();
        let pkey = (path.to_path_buf(), max_px);
        if max_px != 0 {
            if let Some(img) = self.cached_preview_input(&pkey) {
                return Ok((img, 0));
            }
        }
        // A frame parked on disk by `park_working` beats everything below it:
        // no network, no decode, just a local read of exactly the buffer the
        // pipeline wants.
        if let Some(img) = self.unpark_working(path, max_px) {
            eprintln!("[perf] pipeline_input {}: parked frame read from disk in {} ms", path.display(), t0.elapsed().as_millis());
            self.store_preview_input(pkey, img.clone());
            return Ok((img, 0));
        }

        if max_px != 0 {
            let (img, is_new) = self.preview_gates.once(
                &pkey,
                || self.cached_preview_input(&pkey),
                || {
                    let fast = true;
                    let key = (path.to_path_buf(), fast);
                    let (full, _) = self.decode_once(key, || {
                        let t_dec = std::time::Instant::now();
                        let linear = self
                            .decoder
                            .decode_linear(path, fast)
                            .with_context(|| format!("decoding {}", path.display()))?;
                        let dec_ms = t_dec.elapsed().as_millis();
                        let t_pro = std::time::Instant::now();
                        let data = to_prophoto(linear.data, linear.primaries);
                        let pro_ms = t_pro.elapsed().as_millis();
                        eprintln!(
                            "[perf] decode closure {}: decode_linear {} ms, to_prophoto {} ms ({}x{})",
                            path.display(),
                            dec_ms,
                            pro_ms,
                            linear.width,
                            linear.height
                        );
                        Ok(Arc::new(ImageBuf::from_data(
                            linear.width,
                            linear.height,
                            data,
                        )))
                    })?;

                    if full.width.max(full.height) <= max_px {
                        self.store_preview_input(pkey.clone(), full.clone());
                        return Ok(full);
                    }

                    let t_down = std::time::Instant::now();
                    let small = Arc::new(downscale(&full, max_px));
                    let down_ms = t_down.elapsed().as_millis();
                    eprintln!(
                        "[perf] pipeline_input {}: downscale from {}x{} to {}x{} took {} ms",
                        path.display(),
                        full.width,
                        full.height,
                        small.width,
                        small.height,
                        down_ms
                    );
                    self.store_preview_input(pkey.clone(), small.clone());
                    Ok(small)
                },
            )?;
            return Ok((img, if is_new { t0.elapsed().as_millis() } else { 0 }));
        }

        // Full-res decode path (export path, max_px == 0)
        let fast = false;
        let key = (path.to_path_buf(), fast);
        let (full, decode_ms) = self.decode_once(key, || {
            let t_dec = std::time::Instant::now();
            let linear = self
                .decoder
                .decode_linear(path, fast)
                .with_context(|| format!("decoding {}", path.display()))?;
            let dec_ms = t_dec.elapsed().as_millis();
            let t_pro = std::time::Instant::now();
            let data = to_prophoto(linear.data, linear.primaries);
            let pro_ms = t_pro.elapsed().as_millis();
            eprintln!(
                "[perf] decode closure {}: decode_linear {} ms, to_prophoto {} ms ({}x{})",
                path.display(),
                dec_ms,
                pro_ms,
                linear.width,
                linear.height
            );
            Ok(Arc::new(ImageBuf::from_data(
                linear.width,
                linear.height,
                data,
            )))
        })?;

        Ok((full, decode_ms))
    }

    /// Decode a photo at most once, however many threads ask at the same time.
    /// The mechanics, and why, live on [`DecodeGates::once`].
    fn decode_once<F>(&self, key: (PathBuf, bool), decode: F) -> Result<(Arc<ImageBuf>, u128)>
    where
        F: FnOnce() -> Result<Arc<ImageBuf>>,
    {
        let t = std::time::Instant::now();
        let (img, decoded) = self.decode_gates.once(
            &key,
            || self.cached_decode(&key),
            || {
                let img = decode()?;
                self.store_decode(key.clone(), img.clone());
                Ok(img)
            },
        )?;
        eprintln!(
            "[perf] decode {}: {} ({} ms)",
            key.0.display(),
            if decoded { "RAW decoded" } else { "decode cache hit" },
            t.elapsed().as_millis()
        );
        Ok((img, if decoded { t.elapsed().as_millis() } else { 0 }))
    }

    fn cached_decode(&self, key: &(PathBuf, bool)) -> Option<Arc<ImageBuf>> {
        let mut cache = self.decoded.lock().unwrap();
        let i = cache.iter().position(|(k, _)| k == key)?;
        // Move to front: the eviction below is plain LRU.
        let entry = cache.remove(i);
        let img = entry.1.clone();
        cache.insert(0, entry);
        Some(img)
    }

    fn store_decode(&self, key: (PathBuf, bool), img: Arc<ImageBuf>) {
        let mut cache = self.decoded.lock().unwrap();
        cache.retain(|(k, _)| k != &key);
        cache.insert(0, (key, img));
        // Budget in BYTES, not entries: one 102 MP frame is ~300 MB at half
        // res and ~1.2 GB at full, so "keep 3" would mean wildly different
        // memory depending on the body. Index 0 is always kept even if it
        // alone is over budget — that's the photo being worked on.
        let mut total = 0usize;
        let mut keep = cache.len();
        for (i, (_, img)) in cache.iter().enumerate() {
            total += img.data.len() * std::mem::size_of::<f32>();
            if i > 0 && total > DECODE_CACHE_BUDGET_BYTES {
                keep = i;
                break;
            }
        }
        cache.truncate(keep);
    }

    fn cached_preview_input(&self, key: &(PathBuf, u32)) -> Option<Arc<ImageBuf>> {
        let mut cache = self.preview_input.lock().unwrap();
        let i = cache.iter().position(|(k, _)| k == key)?;
        let entry = cache.remove(i);
        let img = entry.1.clone();
        cache.insert(0, entry);
        Some(img)
    }

    fn store_preview_input(&self, key: (PathBuf, u32), img: Arc<ImageBuf>) {
        let mut cache = self.preview_input.lock().unwrap();
        cache.retain(|(k, _)| k != &key);
        cache.insert(0, (key, img));
        cache.truncate(PREVIEW_INPUT_CACHE_ENTRIES);
    }

    /// Decode a photo into the cache without rendering it, so stepping to it
    /// costs nothing. Errors are swallowed: a prefetch that fails just means
    /// the real open pays what it would have paid anyway.
    /// Where decoded frames may be parked across restarts. `None` disables it.
    pub fn set_working_dir(&self, dir: Option<PathBuf>) {
        if let Some(d) = &dir {
            let _ = std::fs::create_dir_all(d);
        }
        *self.working_dir.lock().unwrap() = dir;
    }

    /// Park this photo's pipeline input so a restart can skip read and decode.
    ///
    /// Exactly one photo is ever parked — the one open in Develop — so this
    /// clears whatever was there first. Best-effort: failing to park costs
    /// the next launch the read it costs today, nothing more.
    pub fn park_working(&self, path: &Path, max_px: u32) -> Result<()> {
        let Some(dir) = self.working_dir.lock().unwrap().clone() else {
            return Ok(());
        };
        let dest = dir.join(working_name(path, max_px));
        // Already parked, and a park's contents depend only on (photo, size)
        // — it is the pipeline INPUT, decided before any recipe — so a file
        // under this name is by construction the right bytes.
        //
        // Worth the check because the path below both reads and destroys it:
        // `pipeline_input` unparks the frame, `clear_working` then deletes
        // the file it just read, and we re-serialise 34 million floats to
        // write the identical 35 MB back. That ran on every launch, on the
        // critical path of the photo being resumed.
        if parked_file_is_complete(&dest) {
            return Ok(());
        }
        let (img, _) = self.pipeline_input(path, max_px)?;
        self.clear_working();
        let tmp = dest.with_extension("part");
        let mut out = Vec::with_capacity(8 + img.data.len() * 4);
        out.extend_from_slice(&img.width.to_le_bytes());
        out.extend_from_slice(&img.height.to_le_bytes());
        for v in &img.data {
            out.extend_from_slice(&v.to_le_bytes());
        }
        std::fs::write(&tmp, &out)?;
        std::fs::rename(&tmp, &dest)?;
        Ok(())
    }

    /// Drop whatever is parked. Called on leaving Develop — a grid session has
    /// no use for one frame's decode, and it is tens of megabytes.
    pub fn clear_working(&self) {
        let Some(dir) = self.working_dir.lock().unwrap().clone() else {
            return;
        };
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.filter_map(|e| e.ok()) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }

    /// Read a parked frame back, if this photo is the one that was parked.
    fn unpark_working(&self, path: &Path, max_px: u32) -> Option<Arc<ImageBuf>> {
        let dir = self.working_dir.lock().unwrap().clone()?;
        let bytes = std::fs::read(dir.join(working_name(path, max_px))).ok()?;
        parse_parked(&bytes).map(Arc::new)
    }

    pub fn prefetch(&self, path: &Path, max_px: u32) {
        let _ = self.pipeline_input(path, max_px);
    }
}

/// How much decoded, ProPhoto-f32 image data to keep around. Sized so a
/// handful of frames from a high-megapixel body fit: stepping back to the
/// previous photo is the common move in a cull, and it used to re-decode.
const DECODE_CACHE_BUDGET_BYTES: usize = 3 * 1024 * 1024 * 1024;

/// Downscaled pipeline inputs are small (a 2048px frame is ~37 MB), so this
/// one counts entries rather than bytes.
const PREVIEW_INPUT_CACHE_ENTRIES: usize = 8;

/// Recipe → spektrafilm RuntimeParams. Upstream defaults everywhere else —
/// notably `print_exposure_compensation` + `normalize_print_exposure` stay
/// ON (the image-adaptive print normalization the Python reference has).
pub(crate) fn runtime_params(recipe: &Recipe) -> RuntimeParams {
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

#[cfg(test)]
mod perf_probe {
    use super::*;
    use std::time::Instant;

    /// Where a develop actually spends its time, split into the three
    /// parts that have completely different fixes: reading the file (the
    /// library lives on an NFS mount), decoding it (rawler, upstream), and
    /// developing it (ours, now on the GPU). Measured once, it reframed the
    /// whole picture — the develop is ~1% of a cold open.
    ///
    ///   REVEAL_BENCH_RAW=/path/to/file.RAF \
    ///     cargo test --release -p reveal-engine time_split -- --ignored --nocapture
    /// What an export at a web size actually costs.
    ///
    /// `export_jpeg` asks for `max_px = 0`, the full-resolution decode, and
    /// then throws most of those pixels away in a resize. For X-Trans that
    /// decode is Markesteijn 3-pass. This measures the gap against asking for
    /// the size actually wanted.
    ///
    ///   REVEAL_BENCH_RAW=/path/to/file.RAF \
    ///     cargo test --release -p reveal-engine export_cost -- --ignored --nocapture
    #[test]
    #[ignore = "diagnostic; needs a real RAW via REVEAL_BENCH_RAW"]
    fn export_cost() {
        let raw = std::path::PathBuf::from(std::env::var("REVEAL_BENCH_RAW").unwrap_or_default());
        if !raw.exists() {
            eprintln!("set REVEAL_BENCH_RAW to a RAW file");
            return;
        }
        let data_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let luts = std::env::temp_dir().join("reveal-bench-luts");
        let engine = Engine::new(&data_dir, &luts).expect("engine");
        let mut recipe = Recipe::default();
        recipe.engine = "rapid".to_string();

        for max_px in [0u32, 2048] {
            let label = if max_px == 0 { "full (what export asks for)" } else { "2048 (what it keeps)" };
            let t = Instant::now();
            let (_, w, h, _, _) = engine.develop_rgb8(&raw, &recipe, max_px).expect("develop");
            eprintln!("{label:<30} {w}x{h}  {:?}", t.elapsed());
        }
    }

    #[test]
    #[ignore = "diagnostic; needs a real RAW via REVEAL_BENCH_RAW"]
    fn time_split() {
        let raw = std::path::PathBuf::from(
            std::env::var("REVEAL_BENCH_RAW").unwrap_or_default(),
        );
        if !raw.exists() {
            eprintln!("set REVEAL_BENCH_RAW to a RAW file");
            return;
        }
        let data_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let luts = std::env::temp_dir().join("reveal-bench-luts");
        let engine = Engine::new(&data_dir, &luts).expect("engine");

        // Separate network/disk I/O from decode CPU: the library lives on an
        // NFS mount, so "decode is slow" could be either.
        let t = Instant::now();
        let bytes = std::fs::read(&raw).expect("read");
        eprintln!("file read #1: {:?} ({} MB)", t.elapsed(), bytes.len() / 1_048_576);
        let t = Instant::now();
        let _ = std::fs::read(&raw).expect("read");
        eprintln!("file read #2 (OS-cached): {:?}", t.elapsed());

        let mut recipe = Recipe::default();
        recipe.engine = "rapid".to_string();
        recipe.clarity = 15.0;

        // Cold decode (preview size)
        let t = Instant::now();
        let out = engine.develop_rgba8(&raw, &recipe, 2048).expect("develop");
        eprintln!(
            "cold preview: total {:?}  (decode {}ms, render {}ms) {}x{}",
            t.elapsed(), out.decode_ms, out.render_ms, out.width, out.height
        );

        // Warm: same photo again — decode should be cached
        let t = Instant::now();
        let out = engine.develop_rgba8(&raw, &recipe, 2048).expect("develop");
        eprintln!(
            "warm preview: total {:?}  (decode {}ms, render {}ms)",
            t.elapsed(), out.decode_ms, out.render_ms
        );

        // Fresh engine: OS page cache is warm now, decode cache is not, so
        // this is decode CPU with the I/O taken out.
        let engine2 = Engine::new(&data_dir, &luts).expect("engine2");
        let t = Instant::now();
        let out = engine2.develop_rgba8(&raw, &recipe, 2048).expect("develop");
        eprintln!(
            "decode with warm OS cache: total {:?} (decode {}ms, render {}ms)",
            t.elapsed(), out.decode_ms, out.render_ms
        );

        // Full-resolution export path
        let t = Instant::now();
        let (jpeg, w, h) = engine.export_jpeg(&raw, &recipe, 0, 0.0).expect("export");
        eprintln!("export full: {:?} -> {}x{} ({} KB)", t.elapsed(), w, h, jpeg.len() / 1024);
    }
}


