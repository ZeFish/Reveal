//! Harness: cargo run -p reveal-import --example ingest -- <dcim> <archive>
fn main() {
    let mut a = std::env::args().skip(1);
    let (dcim, archive) = (a.next().expect("usage: ingest <dcim> <archive>"), a.next().unwrap());
    let sources = reveal_import::collect_raws(std::path::Path::new(&dcim));
    eprintln!("{} RAWs sur la carte", sources.len());
    let mut progress = |done: usize, total: usize, cur: &str, _path: &str, _dest: &str, _dir: &str| {
        if !cur.is_empty() {
            eprintln!("  [{}/{}] {}", done + 1, total, cur);
        }
    };
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let s = reveal_import::import(&sources, std::path::Path::new(&archive), &cancel, &mut progress).unwrap();
    eprintln!(
        "{} copiés, {} skippés, {} échoués, {} octets, dossiers: {:?}",
        s.copied, s.skipped, s.failed, s.bytes, s.folders
    );
}
