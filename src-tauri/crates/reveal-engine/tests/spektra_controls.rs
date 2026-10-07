//! Does every control the Spektra panel offers change the picture?
//!
//! A slider that writes a recipe field nothing reads is invisible in the UI (it moves, the
//! photo does not). The panel once shipped two of them — "Grain Amount" and "Roughness" fed
//! the Rapid pipeline only. This renders a small synthetic frame through the real Spektra
//! pipeline, moves each declared control away from its default, and requires the render to
//! change.

use reveal_engine::traits::{EngineControl, RenderEngine};
use reveal_engine::{Recipe, SpektraEngine};
use spektrafilm_math::image::ImageBuf;
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

/// Dark surround, a bright light source, a hard edge and a flat mid-grey patch: something
/// for halation, sharpening, grain, diffusion and exposure each to act on.
fn frame() -> ImageBuf {
    let (w, h) = (96u32, 96u32);
    let mut data = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            if fy < 0.12 {
                // A coloured ramp through the upper mid-tones and highlights.
                let v = 0.05 + 3.0 * fx;
                data.extend_from_slice(&[v, v * 0.6, v * 0.3]);
                continue;
            }
            let v = if (fx - 0.3).powi(2) + (fy - 0.3).powi(2) < 0.012 {
                8.0 // clipped light source
            } else if fx > 0.55 && fy < 0.5 {
                0.18 // flat mid-grey
            } else if fx > 0.55 {
                0.02 // dark half beside it: a hard edge
            } else {
                0.04
            };
            data.extend_from_slice(&[v, v * 0.9, v * 0.8]);
        }
    }
    ImageBuf::from_data(w, h, data)
}

fn mean_abs_diff(a: &ImageBuf, b: &ImageBuf) -> f64 {
    assert_eq!(a.data.len(), b.data.len());
    a.data.iter().zip(&b.data).map(|(x, y)| (*x as f64 - *y as f64).abs()).sum::<f64>() / a.data.len() as f64
}

trait CloneBuf {
    fn clone_buf(&self) -> ImageBuf;
}
impl CloneBuf for ImageBuf {
    fn clone_buf(&self) -> ImageBuf {
        ImageBuf::from_data(self.width, self.height, self.data.clone())
    }
}

fn with(base: &Recipe, id: &str, value: serde_json::Value) -> Recipe {
    let mut v = serde_json::to_value(base).unwrap();
    v[id] = value;
    serde_json::from_value(v).unwrap()
}

fn stocks() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(data_dir().join("profiles"))
        .unwrap()
        .filter_map(|e| e.ok()?.path().file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    names.sort();
    names
}

#[test]
fn every_spektra_control_changes_the_render() {
    let engine = SpektraEngine::new(data_dir(), spektrafilm_gpu::select_backend());
    let luts = std::env::temp_dir();
    let input = frame();
    let base = Recipe::default();
    let reference = engine.render(&input, &base, &luts).expect("baseline render");
    let base_json = serde_json::to_value(&base).unwrap();

    let mut silent = Vec::new();
    for control in engine.control_groups().iter().flat_map(|g| g.controls.iter()) {
        let (id, value) = match control {
            EngineControl::Slider { id, min, max, .. } => {
                let default = base_json[id].as_f64().unwrap_or(0.0);
                // Away from the default: the far end of the range, or the near end if the
                // default already sits at the far one.
                let far = if (*max as f64 - default).abs() >= (default - *min as f64).abs() { *max } else { *min };
                // development_time 0 means "off", so any positive value is a change.
                (id.clone(), serde_json::json!(far))
            }
            EngineControl::Toggle { id, .. } => (id.clone(), serde_json::json!(!base_json[id].as_bool().unwrap_or(false))),
            EngineControl::Select { id, .. } => {
                let current = base_json[id].as_str().unwrap_or_default().to_string();
                let other = stocks().into_iter().find(|s| *s != current && s.contains(if id == "paper" { "kodak_endura" } else { "kodak_portra" }));
                match other {
                    Some(name) => (id.clone(), serde_json::json!(name)),
                    None => continue, // no alternative stock of that kind on disk
                }
            }
            _ => continue,
        };
        // A toggle that only gates sub-sliders (glare) needs its sub-sliders on to be seen.
        let mut recipe = with(&base, &id, value.clone());
        if id == "glare_percent" || id == "glare_roughness" || id == "glare_blur" {
            recipe = with(&recipe, "glare", serde_json::json!(true));
        }
        // Controls that only act while another is on, or only on certain stocks:
        // preflash filters need a preflash; development time only exists for
        // the few stocks whose profile holds one curve per duration (Double-X, 2302),
        // which is why the panel disables it for every other film.
        let mut reference = reference.clone_buf();
        if id == "preflash_y_shift" || id == "preflash_m_shift" {
            recipe = with(&recipe, "preflash_exposure", serde_json::json!(0.2));
            reference = engine.render(&input, &with(&base, "preflash_exposure", serde_json::json!(0.2)), &luts).unwrap();
        }
        if id == "development_time_min" {
            recipe = with(&recipe, "film", serde_json::json!("kodak_doublex"));
            reference = engine.render(&input, &with(&base, "film", serde_json::json!("kodak_doublex")), &luts).unwrap();
        }
        if id.starts_with("dir_couplers_") && id != "dir_couplers_active" {
            recipe = with(&recipe, "dir_couplers_active", serde_json::json!(true));
        }
        let out = engine.render(&input, &recipe, &luts).unwrap_or_else(|e| panic!("render with {id}={value}: {e}"));
        let delta = mean_abs_diff(&reference, &out);
        eprintln!("{id:30} = {value:<10} Δ {delta:.6}");
        if delta < 1e-5 {
            silent.push(id);
        }
    }
    assert!(silent.is_empty(), "controls that do not change the render: {silent:?}");
}

/// The same question for Rapid: every slider and toggle it declares must move the picture.
#[test]
fn every_rapid_control_changes_the_render() {
    let engine = reveal_engine::RapidEngine;
    let luts = std::env::temp_dir();
    let input = frame();
    let mut base_json = serde_json::to_value(Recipe::default()).unwrap();
    base_json["engine"] = "rapid".into();
    let base: Recipe = serde_json::from_value(base_json.clone()).unwrap();
    let reference = engine.render(&input, &base, &luts).expect("baseline render");

    let mut silent = Vec::new();
    for control in engine.control_groups().iter().flat_map(|g| g.controls.iter()) {
        let (label, recipe) = match control {
            EngineControl::Slider { id, min, max, .. } => {
                let default = base_json[id].as_f64().unwrap_or(0.0);
                let far = if (*max as f64 - default).abs() >= (default - *min as f64).abs() { *max } else { *min };
                (id.clone(), with(&base, id, serde_json::json!(far)))
            }
            EngineControl::Toggle { id, .. } => (id.clone(), with(&base, id, serde_json::json!(!base_json[id].as_bool().unwrap_or(false)))),
            EngineControl::IndexedSlider { id, index, max, .. } => {
                let mut v = base_json[id].clone();
                let arr = v.as_array_mut().filter(|a| *index < a.len());
                let Some(arr) = arr else { continue };
                arr[*index] = serde_json::json!(*max);
                (format!("{id}[{index}]"), with(&base, id, v))
            }
            _ => continue,
        };
        // Controls that only act while another is on.
        let (recipe, reference) = if label.starts_with("vignette_") && label != "vignette_amount" {
            let on = with(&base, "vignette_amount", serde_json::json!(-50.0));
            let changed = with(&on, &label, serde_json::to_value(&recipe).unwrap()[&label].clone());
            (changed, engine.render(&input, &on, &luts).unwrap())
        } else if label == "grain_roughness" {
            let on = with(&base, "grain_amount", serde_json::json!(0.5));
            let changed = with(&on, &label, serde_json::to_value(&recipe).unwrap()[&label].clone());
            (changed, engine.render(&input, &on, &luts).unwrap())
        } else {
            (recipe, reference.clone_buf())
        };
        let out = engine.render(&input, &recipe, &luts).unwrap_or_else(|e| panic!("render with {label}: {e}"));
        let delta = mean_abs_diff(&reference, &out);
        eprintln!("rapid {label:30} Δ {delta:.6}");
        if delta < 1e-5 {
            silent.push(label);
        }
    }
    assert!(silent.is_empty(), "Rapid controls that do not change the render: {silent:?}");
}

// The three tonal zones carry the same adjustments as the global layer; what each field does
// inside its zone, and that nothing leaks outside it, is pinned by the zone tests in
// `src/rapid.rs` (`zone_adjustments_act_inside_their_zone`, `a_zone_never_touches_pixels_outside_it`).
