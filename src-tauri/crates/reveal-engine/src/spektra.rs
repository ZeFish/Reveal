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
    template: Mutex<Option<((String, String, u32, u32, u32, u32), Pipeline)>>,
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
                        label: "Exposition".to_string(),
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
                label: "Émulsions".to_string(),
                controls: vec![
                    EngineControl::Select {
                        id: "film".to_string(),
                        label: "Film".to_string(),
                        options_type: "films".to_string(),
                    },
                    EngineControl::Select {
                        id: "paper".to_string(),
                        label: "Papier".to_string(),
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
                label: "Tirages".to_string(),
                controls: vec![
                    EngineControl::Slider {
                        id: "print_exposure_ev".to_string(),
                        label: "Tirage".to_string(),
                        min: -3.0,
                        max: 3.0,
                        step: 0.1,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "y_shift".to_string(),
                        label: "Filtre Y".to_string(),
                        min: -10.0,
                        max: 10.0,
                        step: 1.0,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "m_shift".to_string(),
                        label: "Filtre M".to_string(),
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
                label: "Développement".to_string(),
                controls: vec![
                    // 0 = auto (profile's floor-middle family entry). The
                    // pipeline snaps any value to the nearest entry in the
                    // film's own family, so a plain slider is safe even
                    // though the underlying values are discrete per stock.
                    EngineControl::Slider {
                        id: "development_time_min".to_string(),
                        label: "Durée (min)".to_string(),
                        min: 0.0,
                        max: 20.0,
                        step: 0.5,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "density_gamma".to_string(),
                        label: "Contraste".to_string(),
                        min: -0.5,
                        max: 0.5,
                        step: 0.01,
                        preset: false,
                    },
                ],
            },
            ControlGroup {
                label: "Rendus".to_string(),
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
                        label: "Halo".to_string(),
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
                        label: "Netteté".to_string(),
                        min: 0.0,
                        max: 1.0,
                        step: 0.05,
                        preset: false,
                    },
                    EngineControl::Toggle {
                        id: "glare".to_string(),
                        label: "Éblouissement".to_string(),
                    },
                    EngineControl::Slider {
                        id: "glare_percent".to_string(),
                        label: "Force".to_string(),
                        min: 0.0,
                        max: 0.2,
                        step: 0.005,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "glare_roughness".to_string(),
                        label: "Texture".to_string(),
                        min: 0.0,
                        max: 1.0,
                        step: 0.05,
                        preset: false,
                    },
                    EngineControl::Slider {
                        id: "glare_blur".to_string(),
                        label: "Rayon".to_string(),
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
