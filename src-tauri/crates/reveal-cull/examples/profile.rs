//! Times the local prefilter stage photo by photo, split into its phases, so
//! "AI culling feels stuck" can be answered with numbers instead of guesses.
//!
//!   cargo run --release -p reveal-cull --example profile -- <dir> [count]
use std::path::Path;
use std::time::Instant;

fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn pct(v: &mut Vec<f64>, p: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * p) as usize]
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("usage: profile <dir> [count]");
    let count: usize = args.next().and_then(|c| c.parse().ok()).unwrap_or(40);
    let stress = args.next().as_deref() == Some("stress");
    let mut paths: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "raf" | "dng" | "cr2" | "cr3" | "nef" | "arw"))
        })
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    paths.sort();
    paths.truncate(count);
    println!("{} photos from {dir}", paths.len());
    if stress {
        // Reproduce the stall: the viewer prefetches four full decodes at
        // once while a cull reads thumbnails.
        use reveal_decode::RawDecoder;
        let heavy: Vec<String> = paths.iter().rev().take(8).cloned().collect();
        let t = Instant::now();
        let bg = std::thread::spawn(move || {
            std::thread::scope(|s| {
                for chunk in heavy.chunks(2) {
                    s.spawn(move || {
                        for p in chunk {
                            let _ = reveal_decode::DecoderRegistry.decode_linear(Path::new(p), true);
                        }
                    });
                }
            });
        });
        let survivors = reveal_cull::prefilter(&paths, &reveal_cull::PrefilterConfig::default());
        println!("prefilter under 4 concurrent full decodes: {:.0} ms ({} survivors)", ms(t.elapsed()), survivors.len());
        bg.join().unwrap();
        println!("everything done: {:.0} ms", ms(t.elapsed()));
        return;
    }

    let (mut extract, mut decode, mut metrics, mut bytes) = (vec![], vec![], vec![], vec![]);
    for p in &paths {
        let t = Instant::now();
        let Ok(preview) = reveal_decode::extract_thumb_preview(Path::new(p)) else { continue };
        extract.push(ms(t.elapsed()));
        bytes.push(preview.bytes.len() as f64 / 1024.0);

        let t = Instant::now();
        let Ok(img) = image::load_from_memory(&preview.bytes) else { continue };
        decode.push(ms(t.elapsed()));
        let dims = (img.width(), img.height());

        let t = Instant::now();
        let gray = img.to_luma8();
        let small = reveal_cull::bench::resize_gray(&gray, 480);
        let _ = reveal_cull::bench::sharpness_variance(&small);
        let _ = reveal_cull::bench::exposure_penalty(&small);
        let _ = reveal_cull::bench::dhash(&small);
        metrics.push(ms(t.elapsed()));
        if extract.len() == 1 {
            println!("preview {}x{}", dims.0, dims.1);
        }
    }
    for (name, v) in [("extract (RAW→embedded JPEG)", &mut extract), ("jpeg decode", &mut decode), ("gray+resize+metrics", &mut metrics), ("preview KB", &mut bytes)] {
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        println!("{name:30} mean {mean:8.1}  p50 {:8.1}  p95 {:8.1}", pct(v, 0.5), pct(v, 0.95));
    }

    let t = Instant::now();
    let survivors = reveal_cull::prefilter(&paths, &reveal_cull::PrefilterConfig::default());
    let wall = ms(t.elapsed());
    println!("prefilter() wall, 4 threads: {wall:.0} ms for {} photos = {:.0} ms/photo → {} survivors", paths.len(), wall / paths.len() as f64, survivors.len());

    let t = Instant::now();
    let n = survivors.len().min(80);
    let seq = survivors.iter().take(n).filter(|s| reveal_cull::embedded_preview_jpeg(&s.path, 768).is_some()).count();
    let seq_ms = ms(t.elapsed());
    let t = Instant::now();
    let survivor_paths: Vec<String> = survivors.iter().take(n).map(|s| s.path.clone()).collect();
    let par = reveal_cull::embedded_previews(&survivor_paths, 768).len();
    println!("re-extract+encode 768px x{n}: sequential {seq_ms:.0} ms ({seq} ok) vs embedded_previews {:.0} ms ({par} ok)", ms(t.elapsed()));
}
