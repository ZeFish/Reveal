//! Exposure-response curve, to A/B against the Python reference:
//!   cargo run --release -p reveal-engine --example evcurve -- <raw> [film] [paper] [ae]
//!
//! Prints mean/median output luminance per EV. Compare the SHAPE against
//! scripts/ev_curve.py — decoders differ (libraw vs rawpy) so absolute levels
//! may sit slightly apart, but the response to exposure must match.
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let mut a = std::env::args().skip(1);
    let raw = PathBuf::from(a.next().expect("usage: evcurve <raw> [film] [paper] [ae]"));
    let film = a.next().unwrap_or_else(|| "kodak_gold_200".into());
    let paper = a.next().unwrap_or_else(|| "kodak_portra_endura".into());
    let ae = a.next().map(|s| s == "ae").unwrap_or(false);

    let data_dir = std::env::var("REVEAL_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"));
    let luts_dir = data_dir.join("user_luts");
    let engine = reveal_engine::Engine::new(data_dir, luts_dir)?;

    println!("# rust film={film} paper={paper} auto_exposure={ae}");
    println!("EV\tmean\tmedian");
    for ev in [-2.0f32, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0] {
        let mut recipe = reveal_engine::Recipe::default();
        recipe.film = film.clone();
        recipe.paper = paper.clone();
        recipe.auto_exposure = ae;
        recipe.exposure_ev = ev;
        let (rgb8, w, h, _, _) = engine.develop_rgb8(&raw, &recipe, 700)?;
        let n = (w * h) as usize;
        let mut lums: Vec<f32> = Vec::with_capacity(n);
        for p in rgb8.chunks_exact(3) {
            lums.push(
                (0.2126 * p[0] as f32 + 0.7152 * p[1] as f32 + 0.0722 * p[2] as f32) / 255.0,
            );
        }
        let mean: f32 = lums.iter().sum::<f32>() / lums.len() as f32;
        lums.sort_by(|x, y| x.partial_cmp(y).unwrap());
        let median = lums[lums.len() / 2];
        println!("{ev}\t{mean:.5}\t{median:.5}");
    }
    Ok(())
}
