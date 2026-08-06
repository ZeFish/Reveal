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
