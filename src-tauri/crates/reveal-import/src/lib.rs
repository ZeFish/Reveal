//! Card ingest — find mounted cards, copy their RAWs into the archive's
//! dated layout `<archive>/%Y/%Y-%m-%d/` (the same convention the Python
//! `emulsion.ingest` used, so the new frames land beside the old ones).
//!
//! Copy discipline: temp + rename (a torn copy never looks like a photo),
//! dedupe by SHA-256 (re-running an import is free; a same-name collision
//! with different bytes is filed as `name_1`, `name_2`, … instead of
//! clobbering), never touch the card beyond reads.

use std::path::{Path, PathBuf};

use chrono::TimeZone;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(serde::Serialize, Clone)]
pub struct Card {
    /// Volume mount point, e.g. /Volumes/X100F.
    pub volume: String,
    pub name: String,
    /// The DCIM directory to ingest from.
    pub dcim: String,
    pub raw_count: usize,
}

#[derive(serde::Serialize, Clone)]
pub struct ImportStats {
    pub copied: usize,
    pub skipped: usize,
    pub failed: usize,
    pub bytes: u64,
    /// Dated folders touched, sorted (last = most recent day).
    pub folders: Vec<String>,
    pub ms: u128,
    /// The user hit stop — counts above cover what completed before that.
    pub cancelled: bool,
}

/// Progress callback: (done, total, current file name, current source path, dest path, dest dir).
/// The src path is the file being copied right now — the UI reads its embedded
/// JPEG for a live thumbnail; empty on the final "done" tick.
pub type Progress<'a> = &'a mut dyn FnMut(usize, usize, &str, &str, &str, &str);

/// Removable volumes carrying a DCIM directory, with their RAW counts.
pub fn find_cards() -> Vec<Card> {
    let Ok(volumes) = std::fs::read_dir("/Volumes") else {
        return Vec::new();
    };
    let mut cards = Vec::new();
    for v in volumes.flatten() {
        let dcim = v.path().join("DCIM");
        if !dcim.is_dir() {
            continue;
        }
        let raws = collect_raws(&dcim);
        if raws.is_empty() {
            continue;
        }
        cards.push(Card {
            volume: v.path().to_string_lossy().into_owned(),
            name: v.file_name().to_string_lossy().into_owned(),
            dcim: dcim.to_string_lossy().into_owned(),
            raw_count: raws.len(),
        });
    }
    cards
}

/// All RAW files under a directory (recursive, skips hidden dirs and
/// macOS AppleDouble `._*` metadata files that FAT/exFAT cards collect).
pub fn collect_raws(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if p.is_dir() {
                if !name.starts_with('.') {
                    stack.push(p);
                }
                continue;
            }
            // Skip dotfiles outright — covers macOS AppleDouble `._DSCF3025.RAF`
            // (resource-fork junk macOS writes onto FAT/exFAT cards), plus any
            // other hidden file. Without this, `._X.RAF` passes the extension
            // check below and gets treated as a real photo.
            if name.starts_with('.') {
                continue;
            }
            let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
            if reveal_decode::RAW_EXTENSIONS.contains(&ext.as_str()) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// A source file's resolved destination folder, precomputed once so
/// `import` can order the queue before doing any copying.
struct Planned<'a> {
    src: &'a PathBuf,
    name: String,
    dest_dir: PathBuf,
    /// Cheap existence stat only — the real duplicate-vs-collision call
    /// (size, then SHA-256) still happens in `resolve_dest_lazy`. This just
    /// tells the scheduler which files are certainly new.
    dest_exists: bool,
}

/// Copy `sources` into `<archive>/%Y/%Y-%m-%d/` by capture date (EXIF via
/// libraw; file mtime as fallback). `cancel` is checked between files — an
/// in-flight copy always finishes its temp+rename, so a stop never leaves a
/// torn file in the archive.
///
/// Files are processed certainly-new-first (destination doesn't exist yet),
/// then already-there ones (which need the slower size+SHA-256 duplicate
/// check) last. `collect_raws` sorts by filename, so on a card with
/// sequential frame numbers the freshest shot sorts LAST — on a big
/// re-import of an already-largely-archived card, that meant today's new
/// frame sat behind a long queue of slow duplicate-hash checks on old
/// files, and never got copied if the run was stopped or closed before
/// reaching the end. Reordering costs nothing extra: the destination stat
/// this needs is the same one `resolve_dest_lazy` would do first anyway.
pub fn import(
    sources: &[PathBuf],
    archive: &Path,
    cancel: &std::sync::atomic::AtomicBool,
    progress: Progress<'_>,
) -> Result<ImportStats, ImportError> {
    let t = std::time::Instant::now();
    let total = sources.len();
    let mut stats = ImportStats {
        copied: 0,
        skipped: 0,
        failed: 0,
        bytes: 0,
        folders: Vec::new(),
        ms: 0,
        cancelled: false,
    };

    let mut planned: Vec<Planned> = sources
        .iter()
        .map(|src| {
            let name = src
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let ts = reveal_decode::capture_timestamp(src)
                .or_else(|| {
                    std::fs::metadata(src)
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64)
                })
                .unwrap_or(0);
            let day = chrono::Local
                .timestamp_opt(ts, 0)
                .single()
                .unwrap_or_else(|| chrono::Local.timestamp_opt(0, 0).single().unwrap());
            let dest_dir = archive
                .join(day.format("%Y").to_string())
                .join(day.format("%Y-%m-%d").to_string());
            let dest_exists = dest_dir.join(&name).exists();
            Planned { src, name, dest_dir, dest_exists }
        })
        .collect();
    planned.sort_by_key(|p| p.dest_exists);

    for (i, plan) in planned.iter().enumerate() {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            stats.cancelled = true;
            break;
        }
        let src = plan.src;
        let name = &plan.name;
        let dest_dir = &plan.dest_dir;
        progress(i, total, name, &src.to_string_lossy(), "", "");

        let src_size = std::fs::metadata(src).map(|m| m.len()).unwrap_or(0);

        // Resolve the final destination lazily (only hashes if candidate file exists)
        let (dest, skipped) = match resolve_dest_lazy(&dest_dir, &name, src) {
            Ok((path, true)) => (path, true),
            Ok((path, false)) => (path, false),
            Err(e) => {
                eprintln!("import {name}: resolve dest: {e}");
                stats.failed += 1;
                continue;
            }
        };
        if skipped {
            stats.skipped += 1;
            continue;
        }

        let result = (|| -> std::io::Result<()> {
            std::fs::create_dir_all(&dest_dir)?;
            let tmp_name = dest
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| format!(".{name}.part"));
            let tmp = dest_dir.join(format!(".{tmp_name}.part"));
            std::fs::copy(src, &tmp)?;
            std::fs::rename(&tmp, &dest)?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                stats.copied += 1;
                stats.bytes += src_size;
                let folder = dest_dir.to_string_lossy().into_owned();
                if !stats.folders.contains(&folder) {
                    stats.folders.push(folder.clone());
                }
                progress(
                    stats.copied,
                    total,
                    &name,
                    &src.to_string_lossy(),
                    &dest.to_string_lossy(),
                    &folder,
                );
            }
            Err(e) => {
                eprintln!("import {name}: {e}");
                stats.failed += 1;
            }
        }
    }
    progress(total, total, "", "", "", "");
    stats.folders.sort();
    stats.ms = t.elapsed().as_millis();
    Ok(stats)
}

/// Streaming SHA-256 of a file, hex-encoded. Hashing is only ever called
/// when the destination already exists (re-import or name collision), so
/// the cost is paid rarely; 8 KB is the standard throughput sweet spot.
fn sha256(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    let mut file = std::fs::File::open(path)?;
    let mut buf = vec![0u8; 8 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Resolve where a source file should land inside `dest_dir`, given the
/// intended `name` (e.g. `DSCF3025.RAF`) and the source path `src`.
///
/// Deduplication is lazy:
///   1. If candidate path doesn't exist → fast-path return (0 disk reads/hashes).
///   2. If candidate path exists → compare file sizes first.
///   3. Only compute SHA-256 when sizes match to confirm true duplicate.
fn resolve_dest_lazy(
    dest_dir: &Path,
    name: &str,
    src: &Path,
) -> std::io::Result<(PathBuf, bool)> {
    let candidate = dest_dir.join(name);
    match std::fs::metadata(&candidate) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // New photo — fast path!
            Ok((candidate, false))
        }
        Err(e) => Err(e),
        Ok(dest_meta) => {
            let src_meta = std::fs::metadata(src)?;
            if src_meta.len() == dest_meta.len() {
                let src_hash = sha256(src)?;
                let dest_hash = sha256(&candidate)?;
                if dest_hash.eq_ignore_ascii_case(&src_hash) {
                    return Ok((candidate, true));
                }
            }
            // Collision — find a free suffix slot.
            let mut src_hash_cache: Option<String> = None;
            let (stem, ext) = split_ext(name);
            for i in 1..=9999u32 {
                let cand_name = format!("{stem}_{i}.{ext}");
                let cand = dest_dir.join(&cand_name);
                match std::fs::metadata(&cand) {
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        return Ok((cand, false));
                    }
                    Err(e) => return Err(e),
                    Ok(cand_meta) => {
                        if src_meta.len() == cand_meta.len() {
                            let src_hash = match &src_hash_cache {
                                Some(h) => h.clone(),
                                None => {
                                    let h = sha256(src)?;
                                    src_hash_cache = Some(h.clone());
                                    h
                                }
                            };
                            let cand_hash = sha256(&cand)?;
                            if cand_hash.eq_ignore_ascii_case(&src_hash) {
                                return Ok((cand, true));
                            }
                        }
                    }
                }
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("no free slot for {name} after 9999 collisions"),
            ))
        }
    }
}

/// Split `name` into `(stem, extension)` without the dot. Files with no
/// extension keep the whole name as stem and an empty extension; we only
/// use this for synthesising suffixed names, never for filtering by type.
fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(0) => (name, ""),
        Some(i) => (&name[..i], &name[i + 1..]),
        None => (name, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    /// A fresh scratch dir under the system temp dir, removed on drop.
    struct TempDir(PathBuf);
    impl TempDir {
        fn new(label: &str) -> Self {
            static COUNTER: AtomicUsize = AtomicUsize::new(0);
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "reveal-import-test-{label}-{}-{n}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// A file whose only metadata `import` can read is its mtime
    /// (`capture_timestamp` fails on non-RAW bytes), pinned to a specific
    /// day so every test file lands in the same dated folder regardless of
    /// when the test happens to run.
    fn write_source(dir: &Path, name: &str, contents: &[u8]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn new_files_are_processed_before_existing_duplicates() {
        let card = TempDir::new("card");
        let archive = TempDir::new("archive");

        // Alphabetically, "a_already_imported.raf" sorts before
        // "z_brand_new.raf" — collect_raws would hand `import` exactly this
        // order, mimicking a card where the already-archived frame has a
        // lower sequence number than today's freshly-shot one.
        let dup_src = write_source(card.path(), "a_already_imported.raf", b"same bytes");
        let new_src = write_source(card.path(), "z_brand_new.raf", b"never seen before");

        // Pre-seed the archive with a file byte-identical to dup_src, at the
        // dated folder both files' mtime-derived fallback timestamp resolves
        // to (both were just written, so both fall under "today").
        let today = chrono::Local::now();
        let dest_dir = archive
            .path()
            .join(today.format("%Y").to_string())
            .join(today.format("%Y-%m-%d").to_string());
        std::fs::create_dir_all(&dest_dir).unwrap();
        std::fs::write(dest_dir.join("a_already_imported.raf"), b"same bytes").unwrap();

        let sources = vec![dup_src, new_src];
        let cancel = AtomicBool::new(false);
        let mut order: Vec<String> = Vec::new();
        let stats = import(&sources, archive.path(), &cancel, &mut |_done, _total, current, _src, dest, _dest_dir| {
            if !dest.is_empty() {
                order.push(current.to_string());
            }
        })
        .unwrap();

        assert_eq!(stats.copied, 1, "only the brand-new file should copy");
        assert_eq!(stats.skipped, 1, "the byte-identical file should be recognized as already there");
        assert_eq!(
            order,
            vec!["z_brand_new.raf"],
            "the new file must be the one that actually copies, despite sorting last alphabetically"
        );
        assert!(dest_dir.join("z_brand_new.raf").exists());
    }

    #[test]
    fn stopping_mid_import_still_lands_the_new_file_first() {
        // The scenario the bug report described: a long queue of slow
        // duplicate checks ahead of today's shot in filename order. If the
        // run is stopped (or the app quits) before reaching the end, the
        // new file must already be safely copied — not last in line.
        let card = TempDir::new("card2");
        let archive = TempDir::new("archive2");

        let mut sources = Vec::new();
        for i in 0..5 {
            sources.push(write_source(
                card.path(),
                &format!("a_old_{i}.raf"),
                format!("old bytes {i}").as_bytes(),
            ));
        }
        let new_src = write_source(card.path(), "z_new.raf", b"today's shot");
        sources.push(new_src);

        let today = chrono::Local::now();
        let dest_dir = archive
            .path()
            .join(today.format("%Y").to_string())
            .join(today.format("%Y-%m-%d").to_string());
        std::fs::create_dir_all(&dest_dir).unwrap();
        for i in 0..5 {
            std::fs::write(
                dest_dir.join(format!("a_old_{i}.raf")),
                format!("old bytes {i}").as_bytes(),
            )
            .unwrap();
        }

        // Trip the cancel flag as soon as the FIRST file finishes — with the
        // old filename-ascending order that first file would have been
        // "a_old_0.raf" (a slow duplicate check), leaving z_new.raf
        // uncopied. With new-first scheduling it must be z_new.raf itself.
        let cancel = AtomicBool::new(false);
        let mut copied_first: Option<String> = None;
        {
            let mut report = |_d: usize, _t: usize, current: &str, _s: &str, dest: &str, _dd: &str| {
                if !dest.is_empty() && copied_first.is_none() {
                    copied_first = Some(current.to_string());
                    cancel.store(true, Ordering::Relaxed);
                }
            };
            let stats = import(&sources, archive.path(), &cancel, &mut report).unwrap();
            assert!(stats.cancelled);
            assert_eq!(stats.copied, 1);
        }
        assert_eq!(copied_first.as_deref(), Some("z_new.raf"));
        assert!(dest_dir.join("z_new.raf").exists());
    }
}
