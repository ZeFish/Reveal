//! M1 verification harness: develop one RAW through the film pipeline.
//!
//!   cargo run --release -p reveal-engine --example develop -- input.raf out.jpg [max_px] [film] [paper]
//!
//! `REVEAL_DATA_DIR` overrides the spektrafilm data dir (default:
//! src-tauri/data relative to the workspace).

use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let input = PathBuf::from(args.next().expect("usage: develop <input> <out.jpg> [max_px] [film] [paper]"));
    let output = PathBuf::from(args.next().expect("missing output path"));
    let max_px: u32 = args.next().map(|s| s.parse().unwrap()).unwrap_or(0);

    let mut recipe = reveal_engine::Recipe::default();
    if let Some(f) = args.next() {
        recipe.film = f;
    }
    if let Some(p) = args.next() {
        recipe.paper = p;
    }
    // Deterministic mode for pixel-diff verification.
    if std::env::var("REVEAL_NO_GRAIN").is_ok() {
        recipe.grain = 0.0;
        recipe.glare = false;
    }

    let data_dir = std::env::var("REVEAL_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data")
        });

    let luts_dir = data_dir.join("user_luts");
    let engine = reveal_engine::Engine::new(data_dir, luts_dir)?;
    eprintln!("backend: {}", engine.backend_name());
    eprintln!("recipe:  {} × {}", recipe.film, recipe.paper);

    // REVEAL_BENCH=1: simulate a slider drag — same photo, same stocks,
    // three print exposures in one process. Measures the warm per-move cost
    // (decode + template cached; only with_params + GPU process pays).
    if std::env::var("REVEAL_BENCH").is_ok() {
        for ev in [0.0f32, 0.3, -0.3] {
            let mut r = recipe.clone();
            r.print_exposure_ev = ev;
            let out = engine.develop_jpeg(&input, &r, max_px)?;
            eprintln!(
                "bench print_ev={ev:+.1}: decode {} ms, pipeline {} ms",
                out.decode_ms, out.render_ms
            );
        }
    }

    let (w, h, decode_ms, render_ms) = if output.extension().and_then(|e| e.to_str()) == Some("png")
    {
        let (rgb8, w, h, d, r) = engine.develop_rgb8(&input, &recipe, max_px)?;
        image::save_buffer(&output, &rgb8, w, h, image::ExtendedColorType::Rgb8)?;
        (w, h, d, r)
    } else {
        let out = engine.develop_jpeg(&input, &recipe, max_px)?;
        std::fs::write(&output, &out.jpeg)?;
        (out.width, out.height, out.decode_ms, out.render_ms)
    };
    eprintln!(
        "{}x{}  decode {} ms  pipeline {} ms  → {}",
        w,
        h,
        decode_ms,
        render_ms,
        output.display()
    );
    Ok(())
}
