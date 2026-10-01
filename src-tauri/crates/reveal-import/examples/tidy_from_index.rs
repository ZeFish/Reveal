//! Dry run of the tidy plan on the photos an index already knows about: no file is
//! opened, moved or written. `cargo run -p reveal-import --example tidy_from_index
//! -- <index.sqlite> <folder to tidy> <base> [pattern]`
use reveal_import::tidy::*;
use std::path::{Path, PathBuf};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (db, root, base) = (&a[1], &a[2], &a[3]);
    let pattern = a.get(4).map(String::as_str).unwrap_or("%Y/%Y-%m-%d");
    let conn = rusqlite::Connection::open(db).unwrap();
    let mut stmt = conn
        .prepare("SELECT path, capture_at FROM frames WHERE dir = ?1 OR substr(dir, 1, length(?1) + 1) = ?1 || '/'")
        .unwrap();
    let photos: Vec<Photo> = stmt
        .query_map([root], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?)))
        .unwrap()
        .flatten()
        .map(|(path, ts)| {
            let ts = ts.filter(|t| *t > 0);
            Photo { path: PathBuf::from(path), ts, dating: if ts.is_some() { Dating::Exif } else { Dating::None }, companions: vec![] }
        })
        .collect();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let cfg = Config { root: Path::new(root), base: Path::new(base), pattern, now };
    let plan = plan(&cfg, photos, 0, &|_| false);
    let s = &plan.summary;
    println!("photos {}  in place {}  would move {}  kept (named folders) {}  conflicts {}  undated {} (future {})  new day folders {}",
        s.photos, s.in_place, s.to_move, s.kept, s.conflicts, s.undated, s.future_dated, s.new_folders);
    let mut reasons = std::collections::BTreeMap::new();
    let mut kept = std::collections::BTreeMap::new();
    for i in &plan.items {
        match &i.verdict {
            Verdict::Move { reason } => *reasons.entry(format!("{reason:?}")).or_insert(0usize) += 1,
            Verdict::Kept { folder } => *kept.entry(folder.clone()).or_insert(0usize) += 1,
            _ => {}
        }
    }
    println!("moves by reason: {reasons:?}");
    let mut k: Vec<_> = kept.into_iter().collect();
    k.sort_by(|a, b| b.1.cmp(&a.1));
    println!("kept, biggest named folders: {:?}", &k[..k.len().min(5)]);
    for i in plan.items.iter().filter(|i| matches!(i.verdict, Verdict::Move { .. })).take(4) {
        println!("  e.g. {} -> {}", i.from.display(), i.to.as_ref().unwrap().display());
    }
}
