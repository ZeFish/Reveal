//! Harness: cargo run -p reveal-index --example scan -- <root> <db>
fn main() {
    let mut a = std::env::args().skip(1);
    let (root, db) = (a.next().expect("usage: scan <root> <db>"), a.next().unwrap());
    let idx = reveal_index::Index::open(std::path::Path::new(&db)).unwrap();
    let s = idx.scan(std::path::Path::new(&root)).unwrap();
    eprintln!(
        "{} frames ({} nouveaux, {} retirés), {} dossiers, {} ms",
        s.frames, s.added, s.removed, s.dirs, s.ms
    );
    for d in idx.dirs().unwrap() {
        eprintln!("  {} ({})", d.dir, d.count);
    }
    let dirs = idx.dirs().unwrap();
    if let Some(d) = dirs.first() {
        let f = idx.frames(&d.dir, 0).unwrap();
        eprintln!("premier dossier: {} frames, ex: {:?} r{}", f.len(), f[0].name, f[0].rating);
    }
}
