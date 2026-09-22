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

/// The foldering used when preferences say nothing. Same shape the crate
/// hardcoded before the setting was wired through.
pub const DEFAULT_DATE_FORMAT: &str = "%Y/%Y-%m-%d";

/// Expand a `strftime` folder pattern under `archive`.
///
/// The pattern is a user preference, so it is treated as untrusted: the
/// expansion is split on `/` and every component that could climb out of the
/// archive (`..`, `.`, empty, or an absolute root) is dropped. `PathBuf::join`
/// would otherwise happily accept `/etc` or `../..` and write outside the
/// archive entirely. If nothing survives, fall back to the default rather
/// than dumping every import loose in the archive root.
fn dated_dir(archive: &Path, format: &str, day: chrono::DateTime<chrono::Local>) -> PathBuf {
    let expand = |f: &str| {
        day.format(f)
            .to_string()
            .split('/')
            .filter(|c| !c.is_empty() && *c != "." && *c != "..")
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
    };
    let mut parts = expand(format);
    if parts.is_empty() {
        parts = expand(DEFAULT_DATE_FORMAT);
    }
    let mut dir = archive.to_path_buf();
    for part in parts {
        dir.push(part);
    }
    dir
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
    date_format: &str,
    hashes: &dyn HashCache,
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
            let dest_dir = dated_dir(archive, date_format, day);
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
        let (dest, skipped) = match resolve_dest_lazy(&dest_dir, &name, src, hashes) {
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
            // Hash the source now, while the card is still mounted and its
            // pages are warm from the copy we just did, and remember it
            // against the destination. That is what makes the NEXT import of
            // this card free on the archive side: the expensive read is the
            // one coming back off the NAS, and it never has to happen.
            // Best-effort — a photo that copied fine must not fail because
            // bookkeeping did.
            match sha256(src) {
                Ok(h) => hashes.put(&dest, &h),
                Err(e) => eprintln!("import {name}: hash for cache: {e}"),
            }
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

/// Where the importer remembers what it has already hashed.
///
/// The archive lives on a NAS: reading one 43 MB frame back costs 2.11s, and
/// confirming a duplicate the honest way means reading it in full. Held in
/// the catalogue instead, that read is paid once in a file's life rather than
/// once per import — which is what makes a cryptographic answer affordable
/// here at all.
///
/// The importer never depends on the catalogue directly; the host passes one
/// of these in. `NoHashCache` is the degenerate implementation and keeps the
/// importer usable on its own.
pub trait HashCache: Sync {
    /// Remembered hash for an archived file, or `None` for "not known yet".
    fn get(&self, path: &Path) -> Option<String>;
    /// Remember one. Best-effort: a failure here costs speed, never
    /// correctness, so it must not abort an import.
    fn put(&self, path: &Path, hash: &str);
}

/// A cache that remembers nothing — every comparison reads both files.
pub struct NoHashCache;
impl HashCache for NoHashCache {
    fn get(&self, _path: &Path) -> Option<String> {
        None
    }
    fn put(&self, _path: &Path, _hash: &str) {}
}

/// Streaming SHA-256 of a file, hex-encoded.
fn sha256(path: &Path) -> std::io::Result<String> {
    use sha2::Digest;
    use std::io::Read;
    let mut hasher = sha2::Sha256::new();
    let mut file = std::fs::File::open(path)?;
    let mut buf = vec![0u8; 256 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let mut out = String::with_capacity(64);
    for b in hasher.finalize() {
        out.push_str(&format!("{b:02x}"));
    }
    Ok(out)
}

/// How much of each end of a file is compared to decide "same photo".
/// Covers a file shorter than 2×this entirely, which makes the comparison
/// exact for sidecars and small JPEGs.
const SAMPLE_BYTES: u64 = 1024 * 1024;

/// Do these two files hold the same photo?
///
/// This used to be a full SHA-256 of both. That is the honest answer, but
/// measured on Francis's setup it costs 2.11s to read one 43 MB frame back
/// off the NAS against 0.18s to hash it — I/O is 12x the hash, and a
/// duplicate means reading BOTH files end to end, ~86 MB, to decide not to
/// copy anything. On a card that is mostly already archived that is the
/// entire cost of an import. A faster hash would have saved ~6%.
///
/// So compare the first and last megabyte instead, plus the size. For two
/// RAW frames the head carries the EXIF capture time, camera serial and
/// frame counter and the start of the embedded preview; the tail is deep in
/// entropy-coded sensor data. Two DIFFERENT photographs agreeing on all of
/// that, at identical byte size, under the same filename, does not happen.
///
/// The trade is real and worth stating: this is a very strong heuristic, not
/// a proof. A full hash is the only proof. Restoring one means comparing the
/// whole stream here — the callers do not change.
fn files_look_identical(a: &Path, b: &Path, size: u64) -> std::io::Result<bool> {
    use std::io::{Read, Seek, SeekFrom};

    let mut fa = std::fs::File::open(a)?;
    let mut fb = std::fs::File::open(b)?;
    let span = SAMPLE_BYTES.min(size) as usize;
    let mut buf_a = vec![0u8; span];
    let mut buf_b = vec![0u8; span];

    fa.read_exact(&mut buf_a)?;
    fb.read_exact(&mut buf_b)?;
    if buf_a != buf_b {
        return Ok(false);
    }
    // Already compared the whole file; seeking to the tail would re-read it.
    if size <= SAMPLE_BYTES * 2 {
        return Ok(true);
    }
    let tail = SeekFrom::Start(size - SAMPLE_BYTES);
    fa.seek(tail)?;
    fb.seek(tail)?;
    fa.read_exact(&mut buf_a)?;
    fb.read_exact(&mut buf_b)?;
    Ok(buf_a == buf_b)
}

/// Resolve where a source file should land inside `dest_dir`, given the
/// intended `name` (e.g. `DSCF3025.RAF`) and the source path `src`.
///
/// Deduplication, cheapest test first:
///   1. Candidate path doesn't exist → return immediately, zero reads.
///   2. Sizes differ → definitely a different file.
///   3. Head/tail sample differs → definitely a different file, and we never
///      hashed anything. This is what makes a frame-counter collision cheap.
///   4. Sample matches → prove it with SHA-256. The source is hashed once and
///      reused across the suffix search; the destination's hash comes from
///      the catalogue, and is computed and remembered the first time only.
fn resolve_dest_lazy(
    dest_dir: &Path,
    name: &str,
    src: &Path,
    hashes: &dyn HashCache,
) -> std::io::Result<(PathBuf, bool)> {
    let candidate = dest_dir.join(name);
    if !candidate.exists() {
        // New photo — fast path!
        return Ok((candidate, false));
    }

    let src_meta = std::fs::metadata(src)?;
    let size = src_meta.len();
    let mut src_hash: Option<String> = None;

    let is_same = |dest: &Path, src_hash: &mut Option<String>| -> std::io::Result<bool> {
        let Ok(dest_meta) = std::fs::metadata(dest) else {
            return Ok(false);
        };
        if dest_meta.len() != size {
            return Ok(false);
        }
        if !files_look_identical(src, dest, size)? {
            return Ok(false);
        }
        // Near-certain by now; make it certain.
        let dest_hash = match hashes.get(dest) {
            Some(h) => h,
            None => {
                let h = sha256(dest)?;
                hashes.put(dest, &h);
                h
            }
        };
        let sh = match src_hash {
            Some(h) => h.clone(),
            None => {
                let h = sha256(src)?;
                *src_hash = Some(h.clone());
                h
            }
        };
        Ok(sh == dest_hash)
    };

    if is_same(&candidate, &mut src_hash)? {
        return Ok((candidate, true));
    }

    // Same name, different photo — the frame counter has wrapped, or this
    // card was formatted and reused. Find a free suffix slot, checking each
    // occupant in case THIS one is the duplicate.
    let (stem, ext) = split_ext(name);
    for i in 1..=9999u32 {
        let cand = dest_dir.join(format!("{stem}_{i}.{ext}"));
        if !cand.exists() {
            return Ok((cand, false));
        }
        if is_same(&cand, &mut src_hash)? {
            return Ok((cand, true));
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        format!("no free slot for {name} after 9999 collisions"),
    ))
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

    fn a_day() -> chrono::DateTime<chrono::Local> {
        use chrono::TimeZone;
        chrono::Local.with_ymd_and_hms(2026, 9, 22, 14, 30, 0).unwrap()
    }

    /// The failure that matters: a DIFFERENT photo landing under a name the
    /// archive already holds must never be reported as a duplicate. Francis
    /// formats his cards after importing, so a wrong "already have it" is a
    /// photo lost for good.
    #[test]
    fn a_different_file_of_the_same_size_is_never_a_duplicate() {
        let dir = TempDir::new("dedup-differs");
        let big = 3 * SAMPLE_BYTES as usize; // forces the tail read too

        // Same size, differing only in the FIRST megabyte (where EXIF lives).
        let mut a = vec![7u8; big];
        let mut b = vec![7u8; big];
        b[128] = 9;
        std::fs::write(dir.0.join("head.a"), &a).unwrap();
        std::fs::write(dir.0.join("head.b"), &b).unwrap();
        assert!(!files_look_identical(
            &dir.0.join("head.a"),
            &dir.0.join("head.b"),
            big as u64
        )
        .unwrap());

        // Same size, differing only in the LAST megabyte. A head-only check
        // would call these identical.
        b = a.clone();
        b[big - 64] = 9;
        std::fs::write(dir.0.join("tail.b"), &b).unwrap();
        assert!(!files_look_identical(
            &dir.0.join("head.a"),
            &dir.0.join("tail.b"),
            big as u64
        )
        .unwrap());

        // A true copy is still recognised.
        a[0] = 7;
        std::fs::write(dir.0.join("copy.a"), &a).unwrap();
        assert!(files_look_identical(
            &dir.0.join("head.a"),
            &dir.0.join("copy.a"),
            big as u64
        )
        .unwrap());
    }

    /// Below 2×SAMPLE_BYTES the two reads cover the whole file, so the
    /// comparison is exact rather than sampled — no gap in the middle.
    #[test]
    fn small_files_are_compared_in_full() {
        let dir = TempDir::new("dedup-small");
        let n = 4096usize;
        let a = vec![1u8; n];
        let mut b = a.clone();
        b[n / 2] = 2; // dead centre: only a full comparison sees this
        std::fs::write(dir.0.join("a"), &a).unwrap();
        std::fs::write(dir.0.join("b"), &b).unwrap();
        assert!(!files_look_identical(&dir.0.join("a"), &dir.0.join("b"), n as u64).unwrap());
    }

    #[test]
    fn date_patterns_expand_into_nested_folders() {
        let root = Path::new("/archive");
        assert_eq!(
            dated_dir(root, "%Y/%Y-%m-%d", a_day()),
            Path::new("/archive/2026/2026-09-22")
        );
        // A flat pattern, and one with literal text between fields.
        assert_eq!(dated_dir(root, "%Y-%m-%d", a_day()), Path::new("/archive/2026-09-22"));
        assert_eq!(
            dated_dir(root, "%Y/%m/Shoot %d", a_day()),
            Path::new("/archive/2026/09/Shoot 22")
        );
    }

    /// The pattern is a user preference, so it is untrusted input. `join`
    /// treats an absolute component as a new root and `..` as a real climb,
    /// either of which would write outside the archive.
    #[test]
    fn a_date_pattern_can_never_escape_the_archive() {
        let root = Path::new("/archive");
        for pattern in ["../../etc", "/etc/passwd", "%Y/../../..", "./.././..", "..", "/"] {
            let dir = dated_dir(root, pattern, a_day());
            assert!(
                dir.starts_with(root),
                "pattern {pattern:?} escaped to {}",
                dir.display()
            );
            assert!(
                !dir.components().any(|c| c.as_os_str() == ".."),
                "pattern {pattern:?} kept a climbing component: {}",
                dir.display()
            );
        }
    }

    /// An empty result must not dump the whole import loose in the archive
    /// root, where nothing would ever be found by date again.
    #[test]
    fn an_empty_pattern_falls_back_instead_of_flattening() {
        let root = Path::new("/archive");
        assert_eq!(
            dated_dir(root, "", a_day()),
            Path::new("/archive/2026/2026-09-22")
        );
        assert_eq!(dated_dir(root, "/", a_day()), Path::new("/archive/2026/2026-09-22"));
    }

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
        let stats = import(&sources, archive.path(), DEFAULT_DATE_FORMAT, &NoHashCache, &cancel, &mut |_done, _total, current, _src, dest, _dest_dir| {
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
            let stats = import(&sources, archive.path(), DEFAULT_DATE_FORMAT, &NoHashCache, &cancel, &mut report).unwrap();
            assert!(stats.cancelled);
            assert_eq!(stats.copied, 1);
        }
        assert_eq!(copied_first.as_deref(), Some("z_new.raf"));
        assert!(dest_dir.join("z_new.raf").exists());
    }
}
