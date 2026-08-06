//! Harness: cargo run --release -p reveal-engine --example export -- in.raf out.jpg [long_edge] [border_frac]
fn main() -> anyhow::Result<()> {
    let mut a = std::env::args().skip(1);
    let input = std::path::PathBuf::from(a.next().expect("usage: export <raw> <out.jpg> [edge] [border]"));
    let out = a.next().unwrap();
    let edge: u32 = a.next().map(|s| s.parse().unwrap()).unwrap_or(3000);
    let border: f32 = a.next().map(|s| s.parse().unwrap()).unwrap_or(0.04);

    let data_dir = std::env::var("REVEAL_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"));
    let luts_dir = data_dir.join("user_luts");
    let engine = reveal_engine::Engine::new(data_dir, luts_dir)?;
    let t = std::time::Instant::now();
    let (jpeg, w, h) = engine.export_jpeg(&input, &reveal_engine::Recipe::default(), edge, border)?;
    std::fs::write(&out, &jpeg)?;
    eprintln!("{w}x{h} ({} ko) en {} ms → {out}", jpeg.len() / 1024, t.elapsed().as_millis());
    Ok(())
}
