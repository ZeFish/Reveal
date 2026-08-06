// Named develop presets — a serialized `Recipe` the user can save once and
// apply to any photo. The per-photo XMP sidecars are the only other place a
// recipe lives; a preset is just that same JSON, named and kept under
// <app_data>/presets/ (one file per preset, so it is trivially backed up or
// hand-edited).

use std::fs;
use std::path::{Path, PathBuf};

use reveal_engine::Recipe;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresetEntry {
    pub name: String,
    pub recipe: Recipe,
}

/// Filesystem-safe stem for a preset's display name: keep ASCII alphanumerics,
/// map every other run to a single '-'. Empty result → "preset".
fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in name.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let s = out.trim_matches('-').to_string();
    if s.is_empty() {
        "preset".to_string()
    } else {
        s
    }
}

pub fn presets_dir(app_data: &Path) -> PathBuf {
    app_data.join("presets")
}

/// Every saved preset, sorted by display name. Files that can't be read or
/// parsed are skipped rather than failing the whole list.
pub fn list(dir: &Path) -> Vec<PresetEntry> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(preset) = serde_json::from_str::<PresetEntry>(&text) {
                out.push(preset);
            }
        }
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn save(dir: &Path, name: &str, recipe: &Recipe) -> Result<(), String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("preset name is empty".into());
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let entry = PresetEntry {
        name: name.to_string(),
        recipe: recipe.clone(),
    };
    let text = serde_json::to_string_pretty(&entry).map_err(|e| e.to_string())?;
    fs::write(dir.join(format!("{}.json", slugify(name))), text).map_err(|e| e.to_string())
}

pub fn delete(dir: &Path, name: &str) -> Result<(), String> {
    let path = dir.join(format!("{}.json", slugify(name)));
    if path.exists() {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
