//! The controls each engine declares, as a committed snapshot.
//!
//! The Develop panels for Rapid and Spektra are written by hand in Svelte, while the
//! engines declare their controls here. The two used to drift apart without anyone
//! noticing (a curve declared with channels in Rust, hand-built without them in the
//! panel). `apps/reveal/tests/engine-panels.test.js` checks the panels against this
//! snapshot; this test keeps the snapshot honest.
//!
//! Regenerate after changing a declaration:
//!   UPDATE_SNAPSHOT=1 cargo test -p reveal-engine --test control_declarations

use reveal_engine::traits::{ControlGroup, EngineControl};
use std::collections::BTreeSet;

fn ids(groups: &[ControlGroup]) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for control in groups.iter().flat_map(|g| g.controls.iter()) {
        match control {
            EngineControl::Slider { id, .. }
            | EngineControl::IndexedSlider { id, .. }
            | EngineControl::Select { id, .. }
            | EngineControl::Toggle { id, .. } => {
                out.insert(id.clone());
            }
            EngineControl::LutStack { stage, .. } => {
                out.insert(format!("{stage}_luts"));
            }
            EngineControl::Curve { channels, .. } => {
                out.extend(channels.iter().map(|c| c.id.clone()));
            }
            EngineControl::BandMixer { bands, .. } => {
                out.extend(bands.iter().flat_map(|b| b.fields.iter().map(|f| f.id.clone())));
            }
        }
    }
    out
}

#[test]
fn declared_controls_match_the_committed_snapshot() {
    use reveal_engine::traits::RenderEngine;
    let rapid = ids(&reveal_engine::RapidEngine.control_groups());
    // Declaring controls touches neither the backend nor the data directory.
    let spektra = reveal_engine::spektra::SpektraEngine::new(std::path::PathBuf::new(), spektrafilm_gpu::select_backend());
    let spektra = ids(&spektra.control_groups());
    let snapshot = serde_json::json!({ "rapid": rapid, "spektra": spektra });
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tests/engine-controls.snapshot.json");
    let rendered = serde_json::to_string_pretty(&snapshot).unwrap() + "\n";
    if std::env::var("UPDATE_SNAPSHOT").is_ok() {
        std::fs::write(&path, &rendered).unwrap();
    }
    let committed = std::fs::read_to_string(&path).expect("snapshot missing: run with UPDATE_SNAPSHOT=1");
    assert_eq!(rendered, committed, "engine declarations changed: regenerate the snapshot and check the panels");
}
