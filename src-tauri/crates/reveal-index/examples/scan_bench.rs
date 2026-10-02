//! Time a rescan against a COPY of the index, so a network library can be
//! profiled without touching the real database.
//!
//!   cp ~/Library/Application\ Support/co.utopie.reveal/index-rs.sqlite* /tmp/bench/
//!   cargo run --release -p reveal-index --example scan_bench -- /tmp/bench/index-rs.sqlite <folder>
use std::path::Path;
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let db = args.next().expect("db copy");
    let folder = args.next().expect("folder to rescan");
    let index = reveal_index::Index::open(Path::new(&db)).expect("open");
    let t = Instant::now();
    let mut ticks = 0usize;
    let stats = index
        .scan_subtree_with(Path::new(&folder), |_, _| ticks += 1)
        .expect("scan");
    println!(
        "{folder}\n  {} frames, {} new, {} removed, {} folders in {} ms ({} progress ticks, wall {:?})",
        stats.frames, stats.added, stats.removed, stats.dirs, stats.ms, ticks, t.elapsed()
    );
}
