//! Feed every real `.xmp` under a folder through read + write (on copies) with
//! a watchdog, to catch a sidecar the merge cannot handle.
//!
//!   cargo run --release -p reveal-meta --example xmp_stress -- <folder>
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().map(|x| x == "xmp").unwrap_or(false) {
            out.push(p);
        }
    }
}

fn main() {
    let root = PathBuf::from(std::env::args().nth(1).expect("folder"));
    let mut files = Vec::new();
    collect(&root, &mut files);
    let tmp = std::env::temp_dir().join(format!("xmp-stress-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let (mut ok, mut failed, mut slow) = (0, 0, 0);
    for (i, f) in files.iter().enumerate() {
        let original = tmp.join(format!("{i}.RAF"));
        let copy = reveal_meta::sidecar_path(&original);
        if std::fs::copy(f, &copy).is_err() {
            continue;
        }
        let before = std::fs::read(&copy).unwrap_or_default();
        let (tx, rx) = std::sync::mpsc::channel();
        let o = original.clone();
        std::thread::spawn(move || {
            let t = Instant::now();
            let mut sc = reveal_meta::read(&o).ok().flatten().unwrap_or_default();
            sc.rating = Some(3);
            sc.engine = Some("spektra".into());
            sc.engine_settings = Some(serde_json::json!({"film": "kodak_gold_200"}));
            let r = reveal_meta::write(&o, &sc);
            let _ = tx.send((r.is_ok(), t.elapsed()));
        });
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok((true, d)) => {
                ok += 1;
                if d > Duration::from_millis(500) { slow += 1; println!("SLOW {:?} {}", d, f.display()); }
            }
            Ok((false, _)) => { failed += 1; println!("ERROR {} ({} bytes)", f.display(), before.len()); }
            Err(_) => { println!("HANG {}", f.display()); std::process::exit(2); }
        }
    }
    println!("{} files: {ok} ok, {failed} error, {slow} slow", files.len());
    let _ = std::fs::remove_dir_all(&tmp);
}
