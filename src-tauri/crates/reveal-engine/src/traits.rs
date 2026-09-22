//! RenderEngine Trait and EngineRegistry.
//!
//! Provides the plugin seam for Reveal's digital darkroom engines.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use spektrafilm_math::image::ImageBuf;

use crate::Recipe;

/// Individual UI control definition for engine settings.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EngineControl {
    Slider {
        id: String,
        label: String,
        min: f32,
        max: f32,
        step: f32,
        #[serde(default)]
        preset: bool,
    },
    /// A slider bound to one element of a `Vec<f32>` recipe field (e.g. one
    /// band of the 8-channel HSL matrix) rather than a scalar field.
    IndexedSlider {
        id: String,
        index: usize,
        label: String,
        min: f32,
        max: f32,
        step: f32,
    },
    Select {
        id: String,
        label: String,
        options_type: String, // "films" or "papers"
    },
    Toggle {
        id: String,
        label: String,
    },
    LutStack {
        stage: String, // "pre" or "post"
        label: String,
    },
    /// A tone-curve editor. `channels` names the recipe fields it edits, in
    /// the order the editor tabs them — each holds a `Vec<[f32; 2]>` of
    /// control points, not a scalar, so this can't ride on Slider.
    Curve {
        id: String,
        label: String,
        channels: Vec<CurveChannel>,
    },
    /// "Pick a target, then adjust it." One selector row, then one slider per
    /// `channel` writing whichever field the selected `band` names for it.
    ///
    /// The HSL matrix is 8 bands × 3 channels; laid out flat that was 24
    /// sliders in a column, which no one can aim at (Francis, 2026-09-22:
    /// "aucunement agréable à utiliser"). Lightroom and RapidRAW both solve
    /// it the same way — swatches, then three sliders — and the same shape
    /// serves zone tone (3 zones × 3 channels), so it's declared here once
    /// rather than special-cased per group label in the frontend.
    BandMixer {
        label: String,
        bands: Vec<MixerBand>,
        channels: Vec<MixerChannel>,
    },
}

/// One selectable target of a `BandMixer`.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MixerBand {
    pub label: String,
    /// CSS colour for the selector swatch. `None` renders a text chip
    /// instead, which is what a non-colour axis like Shadows/Midtones wants.
    #[serde(default)]
    pub swatch: Option<String>,
    /// The recipe field each channel writes for this band — same length and
    /// order as the mixer's `channels`.
    pub fields: Vec<MixerField>,
}

/// Where one cell of a `BandMixer` (band × channel) stores its value.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MixerField {
    pub id: String,
    /// Element of a `Vec<f32>` recipe field, for fields like `hsl_hue` that
    /// hold one value per band. `None` means `id` is a plain scalar.
    #[serde(default)]
    pub index: Option<usize>,
}

/// One slider row of a `BandMixer`, shared across every band.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MixerChannel {
    pub label: String,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

/// One tab of a curve editor: the recipe field it writes and how to draw it.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CurveChannel {
    pub id: String,
    pub label: String,
    /// CSS custom property the UI should color this channel's line with.
    pub color: String,
}

/// Logical grouping of controls in the dev panel.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ControlGroup {
    pub label: String,
    pub controls: Vec<EngineControl>,
}

/// The core seam every develop engine implements.
pub trait RenderEngine: Send + Sync {
    /// Unique internal identifier (e.g., "spektra", "rapid", "mono").
    fn id(&self) -> &'static str;

    /// Human-readable label for UI pickers (e.g., "Spektra (Analog)", "Rapid (Digital)").
    fn label(&self) -> &'static str;

    /// Return the control groups and UI schema for this engine.
    fn control_groups(&self) -> Vec<ControlGroup> {
        vec![]
    }

    /// Render a scene-linear float RGB `ImageBuf` using the provided recipe.
    fn render(&self, input: &ImageBuf, recipe: &Recipe, luts_dir: &Path) -> Result<ImageBuf>;
}

/// Dynamic registry of available rendering engines.
pub struct EngineRegistry {
    engines: HashMap<&'static str, Arc<dyn RenderEngine>>,
    default_id: &'static str,
}

impl EngineRegistry {
    pub fn new() -> Self {
        Self {
            engines: HashMap::new(),
            default_id: "spektra",
        }
    }

    pub fn register(&mut self, engine: Arc<dyn RenderEngine>) {
        self.engines.insert(engine.id(), engine);
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn RenderEngine>> {
        self.engines
            .get(id)
            .cloned()
            .or_else(|| self.engines.get(self.default_id).cloned())
    }

    pub fn list(&self) -> Vec<EngineInfo> {
        let mut list: Vec<EngineInfo> = self
            .engines
            .values()
            .map(|e| EngineInfo {
                id: e.id().to_string(),
                label: e.label().to_string(),
                control_groups: e.control_groups(),
            })
            .collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        list
    }
}

impl Default for EngineRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata entry returned to UI pickers.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct EngineInfo {
    pub id: String,
    pub label: String,
    pub control_groups: Vec<ControlGroup>,
}
