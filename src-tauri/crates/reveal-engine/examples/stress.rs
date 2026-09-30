//! Fires many renders at once, the way the app does when the viewer, the
//! sidecar publishes and an export overlap. Before the GPU gate this wedged
//! with a dozen in flight (0% CPU, every thread parked in wgpu's Metal
//! encoder). Prints how long N concurrent 2048px developes take, or times out.
//!
//!   cargo run --release -p reveal-engine --example stress -- <dir with RAFs> [threads=16]
use std::sync::Arc;

fn main() -> anyhow::Result<()> {
    let mut a = std::env::args().skip(1);
    let dir = std::path::PathBuf::from(a.next().expect("usage: stress <dir> [threads]"));
    let threads: usize = a.next().and_then(|s| s.parse().ok()).unwrap_or(16);
    let mut raws: Vec<_> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("raf")))
        .collect();
    raws.sort();
    raws.truncate(threads);

    let data_dir = std::env::var("REVEAL_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data"));
    let engine = Arc::new(reveal_engine::Engine::new(data_dir.clone(), data_dir.join("user_luts"))?);
    let done = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let t = std::time::Instant::now();

    let watchdog = {
        let done = done.clone();
        let total = raws.len();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(240));
            if done.load(std::sync::atomic::Ordering::Relaxed) < total {
                eprintln!("WEDGED: {}/{total} finished after 240 s", done.load(std::sync::atomic::Ordering::Relaxed));
                std::process::exit(2);
            }
        })
    };

    let handles: Vec<_> = raws
        .into_iter()
        .map(|raw| {
            let engine = engine.clone();
            let done = done.clone();
            std::thread::spawn(move || {
                let r = engine.develop_jpeg(&raw, &reveal_engine::Recipe::default(), 2048);
                done.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                r.map(|o| o.jpeg.len())
            })
        })
        .collect();
    for h in handles {
        let _ = h.join();
    }
    drop(watchdog);
    eprintln!("{} renders in {} ms", done.load(std::sync::atomic::Ordering::Relaxed), t.elapsed().as_millis());
    Ok(())
}
