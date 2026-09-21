//! Spektra Engine: Physics-based film & paper spectral simulation engine.
//!
//! Encapsulates spektrafilm-rs pipeline creation, stock profile resolution,
//! enlarger illuminant filter shifts, and WGPU compute rendering.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Result;
use spektrafilm_core::pipeline::Pipeline;
use spektrafilm_core::profile;
use spektrafilm_gpu::ComputeBackend;
use spektrafilm_math::image::ImageBuf;

use crate::traits::{ControlGroup, EngineControl, RenderEngine};
use crate::{runtime_params, Recipe};

pub struct SpektraEngine {
    data_dir: PathBuf,
    backend: Box<dyn ComputeBackend>,
    #[allow(clippy::type_complexity)]
    template: Mutex<Option<((String, String, u32, u32, u32, u32, u32, u32, u32), Pipeline)>>,
}

impl SpektraEngine {
    pub fn new(data_dir: PathBuf, backend: Box<dyn ComputeBackend>) -> Self {
        Self {
            data_dir,
            backend,
            template: Mutex::new(None),
        }
    }

    fn pipeline_for(&self, recipe: &Recipe) -> Result<Pipeline> {
        let mut params = runtime_params(recipe);
        let key = (
            recipe.film.clone(),
            recipe.paper.clone(),
            params.enlarger.y_filter_shift.to_bits(),
            params.enlarger.m_filter_shift.to_bits(),
            if params.enlarger.print_exposure_compensation {
                params.camera.exposure_compensation_ev.to_bits()
            } else {
                0f32.to_bits()
            },
            // `development_time` gets resolved into the baked film/paper
            // profile at construction (`resolve_for_render`, called only from
            // `new_with_spectral`) — `with_params` on a cache hit does NOT
            // re-resolve it, so a stale cache would silently ignore the
            // slider. Must be part of the rebuild key.
            recipe.development_time_min.to_bits(),
            // Same trap: `preflash_raw` is computed once by
            // `compute_preflash_raw` inside `new_with_spectral` and just
            // carried forward untouched by `with_params` on a cache hit —
            // all three preflash sliders need to force a rebuild.
            recipe.preflash_exposure.to_bits(),
            recipe.preflash_y_shift.to_bits(),
            recipe.preflash_m_shift.to_bits(),
        );

        let mut guard = self.template.lock().unwrap();
        let rebuild = match guard.as_ref() {
            Some((k, _)) => *k != key,
            None => true,
        };
        if rebuild {
            let film = profile::load_profile_by_name(&self.data_dir, &recipe.film)
                .map_err(|e| anyhow::anyhow!("loading film profile {}: {e}", recipe.film))?;

            // Slide/positive stocks should be scanned directly, bypassing the print stage
            params.io.scan_film = film.is_positive();

            let print = profile::load_profile_by_name(&self.data_dir, &recipe.paper)
                .map_err(|e| anyhow::anyhow!("loading paper profile {}: {e}", recipe.paper))?;
            let pipeline = Pipeline::new_with_spectral(film, print, params.clone(), &self.data_dir)
                .map_err(|e| anyhow::anyhow!("building spectral pipeline: {e}"))?;
            *guard = Some((key, pipeline.clone()));
            Ok(pipeline)
        } else {
            let (_, p) = guard.as_ref().unwrap();
            // Important: params from runtime_params defaults scan_film to false.
            // We must update it from the cached film profile before applying it.
            params.io.scan_film = p.film.is_positive();
            Ok(p.clone().with_params(params))
        }
    }
}

impl RenderEngine for SpektraEngine {
    fn id(&self) -> &'static str {
        "spektra"
    }

    fn label(&self) -> &'static str {
        "Spektra"
    }

    fn control_groups(&self) -> Vec<ControlGroup> {
        vec![
            ControlGroup {
                label: "".to_string(),
                controls: vec![
                    EngineControl::Toggle {
                        id: "auto_exposure".to_string(),
                        label: "Auto exposure".to_string(),
                    },
                    EngineControl::Slider {
                        id: "exposure_ev".to_string(),
                        label: "Exposure".to_string(),
                        min: -3.0,
                        max: 3.0,
                        step: 0.1,
                        preset: false,
                    },
                ],
            },
            // Spektra: Film + Paper ARE the pre-grade (no separate pre-LUTs needed).
            // Post-grading is for finishing passes after print, not engine-level.
            // LUTs are Rapid-only (see RapidEngine::control_groups).
            ControlGroup {
                label: "Emulsions".to_string(),
                controls: vec![
                    EngineControl::Select {
                        id: "film".to_string(),
                        label: "Film".to_string(),
                        options_type: "films".to_string(),
                    },
                    EngineControl::Select {
                        id: "paper".to_string(),
                        label: "Paper".to_string(),
                        options_type: "papers".to_string(),
                    },
                    // Reference spektrafilm-rs GUI uses a 4–120mm log slider
                    // here; the generic dev-panel slider is linear, but the
                    // range matches (35mm ≈ default, up to large format).
                    EngineControl::Slider {
                        id: "film_format_mm".to_string(),
                        label: "Format (mm)".to_string(),
                        min: 4.0,
                        max: 120.0,
                        step: 1.0,
                        preset: false,
                    },
                ],
            },
            ControlGroup {
                label: "Prints".to_string(),
                controls: vec![
                    EngineControl::Slider {
                        id: "print_exposure_ev".to_string(),
                        label: "Print".to_string(),
                        min: -3.0,
                        max: 3.0,
                        step: 0.1,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "y_shift".to_string(),
                        label: "Y Filter".to_string(),
                        min: -10.0,
                        max: 10.0,
                        step: 1.0,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "m_shift".to_string(),
                        label: "M Filter".to_string(),
                        min: -10.0,
                        max: 10.0,
                        step: 1.0,
                        preset: false,
                    },
                    // Whites/highlights/midtones/shadows/rolloff removed: they
                    // are buggy digital tonal tweaks, not part of spektra's
                    // physical enlarger/emulsion model. They live on the rapid
                    // (digital) engine instead.
                ],
            },
            ControlGroup {
                // A small fogging exposure onto the print before the main one,
                // lifting shadow density to compress contrast — a classic
                // darkroom technique for printing a high-contrast negative.
                // Off (0 exposure) by default; the Y/M shifts only matter once
                // it's on.
                label: "Pre-flash".to_string(),
                controls: vec![
                    EngineControl::Slider {
                        id: "preflash_exposure".to_string(),
                        label: "Exposure".to_string(),
                        min: 0.0,
                        max: 0.5,
                        step: 0.01,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "preflash_y_shift".to_string(),
                        label: "Y Filter".to_string(),
                        min: -50.0,
                        max: 50.0,
                        step: 1.0,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "preflash_m_shift".to_string(),
                        label: "M Filter".to_string(),
                        min: -50.0,
                        max: 50.0,
                        step: 1.0,
                        preset: false,
                    },
                ],
            },
            ControlGroup {
                label: "Development".to_string(),
                controls: vec![
                    // 0 = auto (profile's floor-middle family entry). The
                    // pipeline snaps any value to the nearest entry in the
                    // film's own family, so a plain slider is safe even
                    // though the underlying values are discrete per stock.
                    EngineControl::Slider {
                        id: "development_time_min".to_string(),
                        label: "Duration (min)".to_string(),
                        min: 0.0,
                        max: 20.0,
                        step: 0.5,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "density_gamma".to_string(),
                        label: "Contrast".to_string(),
                        min: -0.5,
                        max: 0.5,
                        step: 0.01,
                        preset: false,
                    },
                    // Inter-layer dye interaction during development — real
                    // film chemistry, on by default (spektrafilm-rs's own
                    // default). The three tuning params only matter while active.
                    EngineControl::Toggle {
                        id: "dir_couplers_active".to_string(),
                        label: "DIR Couplers".to_string(),
                    },
                    EngineControl::Slider {
                        id: "dir_couplers_amount".to_string(),
                        label: "Intensity".to_string(),
                        min: 0.0,
                        max: 2.0,
                        step: 0.05,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "dir_couplers_diffusion_size".to_string(),
                        label: "Diffusion (µm)".to_string(),
                        min: 0.0,
                        max: 100.0,
                        step: 1.0,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "dir_couplers_diffusion_tail".to_string(),
                        label: "Tail (µm)".to_string(),
                        min: 0.0,
                        max: 400.0,
                        step: 5.0,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "dir_couplers_tail_weight".to_string(),
                        label: "Tail Weight".to_string(),
                        min: 0.0,
                        max: 1.0,
                        step: 0.01,
                        preset: false,
                    },
                ],
            },
            ControlGroup {
                label: "Rendering".to_string(),
                controls: vec![
                    EngineControl::Slider {
                        id: "halation".to_string(),
                        label: "Halation".to_string(),
                        min: 0.0,
                        max: 1.0,
                        step: 0.05,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "halation_size".to_string(),
                        label: "Halation Size".to_string(),
                        min: 0.5,
                        max: 1.5,
                        step: 0.05,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "diffusion".to_string(),
                        label: "Diffusion".to_string(),
                        min: 0.0,
                        max: 1.5,
                        step: 0.05,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "grain".to_string(),
                        label: "Grain".to_string(),
                        min: 0.0,
                        max: 1.0,
                        step: 0.05,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "sharpen".to_string(),
                        label: "Sharpen".to_string(),
                        min: 0.0,
                        max: 1.0,
                        step: 0.05,
                        preset: false,
                    },
                    EngineControl::Toggle {
                        id: "glare".to_string(),
                        label: "Glare".to_string(),
                    },
                    EngineControl::Slider {
                        id: "glare_percent".to_string(),
                        label: "Amount".to_string(),
                        min: 0.0,
                        max: 0.2,
                        step: 0.005,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "glare_roughness".to_string(),
                        label: "Roughness".to_string(),
                        min: 0.0,
                        max: 1.0,
                        step: 0.05,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "glare_blur".to_string(),
                        label: "Blur".to_string(),
                        min: 0.0,
                        max: 2.0,
                        step: 0.05,
                        preset: false,
                    },
                ],
            },
        ]
    }

    fn render(&self, input: &ImageBuf, recipe: &Recipe, _luts_dir: &Path) -> Result<ImageBuf> {
        // The digital tonal sliders (whites/highlights/midtones/shadows/rolloff)
        // are Rapid-only; Spektra's tonal response comes entirely from the
        // physical enlarger/emulsion model. We deliberately do NOT pre-bake a
        // `apply_tonal_adjustments` pass here — that was a leftover digital
        // grade that conflicted with the spectral simulation (see the note on
        // the removed "Tirages" sliders above).
        let pipeline = self.pipeline_for(recipe)?;
        let result = pipeline
            .process_resident_borrowed(input, self.backend.as_ref())
            .unwrap_or_else(|| pipeline.process(input.clone(), self.backend.as_ref()));
        Ok(result)
    }
}
