//! Library index — SQLite over the archive, so the grid opens folders from
//! a query instead of a filesystem walk. DB file `index-rs.sqlite` (distinct
//! from the archived Swift app's index by design).
//!
//! Lessons carried from the Swift indexer: Synology system dirs (`@eaDir`,
//! `#recycle`, …) are walked-into traps — they hold thumbnail JPEGs that
//! index as phantom frames; they start with `@`/`#`, not `.`, so a
//! hidden-files check misses them.

use rayon::prelude::*;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const SKIP_DIRS: &[&str] = &[
    "@eadir",
    "#recycle",
    "#snapshot",
    "@tmp",
    "@sharebin",
    "@synologydrive",
];

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("sqlite: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(serde::Serialize)]
pub struct ScanStats {
    pub frames: usize,
    pub added: usize,
    pub removed: usize,
    pub dirs: usize,
    pub ms: u128,
}

#[derive(serde::Serialize)]
pub struct DirRow {
    pub dir: String,
    pub count: i64,
}

#[derive(serde::Serialize)]
pub struct FrameRow {
    pub path: String,
    pub name: String,
    pub rating: u8,
    pub capture_at: Option<i64>,
    /// The photo's size as it is SEEN — the sensor rotation already applied
    /// (see `reveal_decode::oriented_dimensions`). `None` for a row indexed
    /// before this column existed, or a file libraw cannot read: the grid
    /// treats that as "shape unknown" and waits for the thumbnail, which is
    /// what it did for every photo until now.
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// A registered library as the management UI needs to see it.
#[derive(serde::Serialize, Clone, Debug)]
pub struct Catalogue {
    pub path: String,
    /// Frames the catalogue holds for it.
    pub frames: usize,
    /// Is the folder reachable right now? False for an unmounted NAS.
    pub online: bool,
}

pub struct Index {
    conn: Mutex<Connection>,
}

impl Index {
    pub fn open(db_path: &Path) -> Result<Self, IndexError> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS frames (
               path TEXT PRIMARY KEY,
               dir TEXT NOT NULL,
               name TEXT NOT NULL,
               mtime INTEGER NOT NULL,
               rating INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS frames_dir ON frames(dir);
             CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT);
             -- Catalogue roots. A library is a SET of roots (each an added
             -- folder), not one — the same NAS can be two mounts, two roots.
             -- `added_at` orders the sidebar; the legacy single `meta.root`
             -- is migrated into here below and kept as the primary.
             CREATE TABLE IF NOT EXISTS roots (
               path TEXT PRIMARY KEY,
               added_at INTEGER NOT NULL DEFAULT 0
             );",
        )?;
        // Migration: capture date (unix secs, from the RAW's EXIF at scan) —
        // the grid sorts by it; filenames lie as soon as two cards mix.
        let _ = conn.execute("ALTER TABLE frames ADD COLUMN capture_at INTEGER", []);
        // Migration: content hash, filled in by the importer. Deduplicating a
        // re-imported card used to read the archived copy back off the NAS in
        // full to compare it — 2.11s for a 43 MB frame, and the whole cost of
        // an import from a card that is mostly already archived. Remembering
        // the hash here pays that read once in a file's life instead of once
        // per import. NULL means "not known yet", never "no hash".
        let _ = conn.execute("ALTER TABLE frames ADD COLUMN content_hash TEXT", []);
        // Migration: the photo's size as seen, read from the RAW header at
        // scan (no pixel decode, and in the same libraw open as capture_at).
        // Lets the grid lay a folder out — including which frames are
        // portrait — before a single thumbnail has come back from the NAS.
        // NULL means "not known yet", for rows indexed before this and for
        // files libraw cannot read.
        let _ = conn.execute("ALTER TABLE frames ADD COLUMN width INTEGER", []);
        let _ = conn.execute("ALTER TABLE frames ADD COLUMN height INTEGER", []);
        let _ = conn.execute("DELETE FROM frames WHERE name LIKE '.%' OR name LIKE '._%'", []);
        // Migration: seed `roots` from the legacy single `meta.root` so an
        // existing catalogue keeps working — first launch after the upgrade
        // finds one root, exactly the old behaviour, then `add_root` extends it.
        let legacy_root: Option<String> = conn
            .query_row("SELECT value FROM meta WHERE key='root'", [], |r| r.get(0))
            .ok();
        if let Some(r) = legacy_root {
            if !r.is_empty() {
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO roots(path, added_at) VALUES(?1, 0)",
                    [&r],
                );
            }
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn root(&self) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row("SELECT value FROM meta WHERE key='root'", [], |r| r.get(0))
            .ok()
    }

    /// Every catalogue root, oldest-added first (then lexical). This is the
    /// real library shape — the sidebar renders one tree per root instead of
    /// guessing catalogues from which folders happen to hold photos directly.
    pub fn roots(&self) -> Result<Vec<String>, IndexError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT path FROM roots ORDER BY added_at, path")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.flatten().collect())
    }

    /// Register a catalogue root (idempotent). Does NOT scan — the caller
    /// scans the subtree afterwards so frames land in the index.
    pub fn add_root(&self, path: &str) -> Result<(), IndexError> {
        let conn = self.conn.lock().unwrap();
        // `added_at = max+1` keeps insertion order without a clock in here.
        conn.execute(
            "INSERT OR IGNORE INTO roots(path, added_at)
             VALUES(?1, COALESCE((SELECT MAX(added_at) FROM roots), 0) + 1)",
            [path],
        )?;
        Ok(())
    }

    /// Drop a catalogue root and prune every frame beneath it — removing a
    /// library forgets its photos from the index (the files are untouched).
    pub fn remove_root(&self, path: &str) -> Result<usize, IndexError> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM roots WHERE path=?1", [path])?;
        let like = format!("{path}/%");
        let n = conn.execute(
            "DELETE FROM frames WHERE path=?1 OR path LIKE ?2",
            rusqlite::params![path, like],
        )?;
        Ok(n)
    }

    /// One registered library, with enough to manage it without opening it.
    ///
    /// `online` matters because a root is usually a NAS mount: a library that
    /// is merely unmounted looks exactly like one whose folder was deleted,
    /// and removing it would silently throw away every rating and story mark
    /// the catalogue holds for it. The UI needs to tell those apart.
    pub fn catalogues(&self) -> Result<Vec<Catalogue>, IndexError> {
        let roots = self.roots()?;
        let conn = self.conn.lock().unwrap();
        let mut out = Vec::with_capacity(roots.len());
        for path in roots {
            let like = format!("{path}/%");
            let frames: i64 = conn
                .query_row(
                    "SELECT count(*) FROM frames WHERE path=?1 OR path LIKE ?2",
                    rusqlite::params![&path, &like],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            let online = Path::new(&path).is_dir();
            out.push(Catalogue { path, frames: frames as usize, online });
        }
        Ok(out)
    }

    /// The registered root that contains `path` (longest match wins for
    /// nested roots) — used to validate a subtree rescan against the set.
    pub fn root_containing(&self, path: &str) -> Result<Option<String>, IndexError> {
        let roots = self.roots()?;
        Ok(roots
            .into_iter()
            .filter(|r| path == r || path.starts_with(&format!("{r}/")))
            .max_by_key(|r| r.len()))
    }

    /// Walk `root` recursively, upsert frames, prune the vanished. Sidecar
    /// ratings are read for new/changed files only (cheap rescan).
    pub fn scan(&self, root: &Path) -> Result<ScanStats, IndexError> {
        self.scan_with(root, |_, _| {})
    }

    /// `scan`, reporting progress. Each directory's frames are committed in
    /// their OWN transaction as the walk reaches them — over a slow mount
    /// (the NFS library) the sidebar fills in live instead of staying empty
    /// until a whole-tree walk lands. `on_progress(dirs_walked, frames_seen)`
    /// fires after every directory; prune runs at the end, scoped to the walk.
    pub fn scan_with(
        &self,
        root: &Path,
        mut on_progress: impl FnMut(usize, usize),
    ) -> Result<ScanStats, IndexError> {
        self.scan_scoped_with(root, true, &mut on_progress)
    }

    /// Reconcile one folder subtree without replacing the catalogue root.
    pub fn scan_subtree_with(
        &self,
        root: &Path,
        mut on_progress: impl FnMut(usize, usize),
    ) -> Result<ScanStats, IndexError> {
        self.scan_scoped_with(root, false, &mut on_progress)
    }

    fn scan_scoped_with(
        &self,
        root: &Path,
        update_root: bool,
        on_progress: &mut impl FnMut(usize, usize),
    ) -> Result<ScanStats, IndexError> {
        let t = std::time::Instant::now();
        let mut stack = vec![root.to_path_buf()];
        let mut dirs = 0usize;
        let mut frames_seen = 0usize;
        let mut added = 0usize;
        let mut found_paths: HashSet<String> = HashSet::new();

        // One snapshot of what's already indexed — the mtime-skip check.
        let known: HashSet<(String, i64)> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT path, mtime FROM frames")?;
            let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
            rows.flatten().collect()
        };
        let root_prefix = root.to_string_lossy().into_owned();
        let known_under_root = known.iter().filter(|(p, _)| p.starts_with(&root_prefix)).count();

        // Real incident: an autofs NFS mount (an idle-unmounted NAS share)
        // was still waking up when a routine reindex ran. `read_dir(root)`
        // failed on the very first stack entry — the root itself — so the
        // walk found nothing, and the prune step below (scoped to "whatever
        // wasn't seen this walk") quietly deleted all 44k previously-indexed
        // frames. The photos on disk were never touched; only the SQLite
        // index was. Failing loudly here, before any pruning happens, turns
        // that into a clear error instead of a silent wipe.
        let mut first = true;
        while let Some(d) = stack.pop() {
            let entries = match std::fs::read_dir(&d) {
                Ok(e) => e,
                Err(e) => {
                    if first {
                        return Err(IndexError::Io(e));
                    }
                    first = false;
                    continue; // a deeper subdir being unreadable is tolerable; prune stays scoped to what we DID see
                }
            };
            first = false;
            dirs += 1;
            let mut batch: Vec<(String, String, String, i64)> = Vec::new(); // path,dir,name,mtime
            for e in entries.flatten() {
                let p = e.path();
                let name = e.file_name().to_string_lossy().to_string();
                let lower = name.to_lowercase();
                if p.is_dir() {
                    if !name.starts_with('.') && !SKIP_DIRS.contains(&lower.as_str()) {
                        stack.push(p);
                    }
                    continue;
                }
                // macOS writes a "._name.raf" AppleDouble sidecar next to
                // every real file on NFS/SMB volumes — same extension, not a
                // photo, and unreadable as one (permanent decode failures).
                if name.starts_with('.') {
                    continue;
                }
                let Some(ext) = lower.rsplit('.').next() else {
                    continue;
                };
                if !reveal_decode::RAW_EXTENSIONS.contains(&ext) {
                    continue;
                }
                let mtime = e
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0);
                batch.push((
                    p.to_string_lossy().into_owned(),
                    d.to_string_lossy().into_owned(),
                    name,
                    mtime,
                ));
            }

            frames_seen += batch.len();
            if !batch.is_empty() {
                // Sidecar + EXIF reads (NFS round-trips) happen OUTSIDE the
                // write lock; only the upserts hold the connection. Both are
                // paid for new/changed files only — the mtime skip covers
                // rescans.
                type Row = (String, String, String, i64, u8, Option<i64>, Option<u32>, Option<u32>);
                let rows: Vec<Row>;
                let to_read: Vec<_> = batch
                    .into_iter()
                    .filter(|(path, _, _, mtime)| {
                        found_paths.insert(path.clone());
                        !known.contains(&(path.clone(), *mtime)) // unchanged: nothing to do
                    })
                    .collect();
                // Two reads per file, both of them a round trip to the NAS: the
                // XMP sidecar for the rating, and the RAW's header for the date
                // and the size. Done one file at a time that is ~99ms of
                // waiting each, measured on a 61-photo folder. They are
                // independent, so wait for them together.
                rows = to_read
                    .into_par_iter()
                    .map(|(path, dir, name, mtime)| {
                        let rating = reveal_meta::read(Path::new(&path))
                            .ok()
                            .flatten()
                            .and_then(|s| s.rating)
                            .unwrap_or(0);
                        // One header open for both — two would double the
                        // round trips over a six-figure library for nothing.
                        let (captured, w, h) = reveal_decode::capture_header(Path::new(&path));
                        (path, dir, name, mtime, rating, captured, w, h)
                    })
                    .collect();
                if !rows.is_empty() {
                    let mut conn = self.conn.lock().unwrap();
                    let tx = conn.transaction()?;
                    for (path, dir, name, mtime, rating, captured, w, h) in &rows {
                        tx.execute(
                            // A remembered content_hash survives a rescan, but
                            // only while mtime is unchanged — a file rewritten
                            // under the same path would otherwise keep an
                            // empty promise of an old hash and make a genuinely
                            // new photo look like a duplicate. Bare `mtime` in
                            // DO UPDATE is the existing row's value.
                            "INSERT INTO frames(path,dir,name,mtime,rating,capture_at,width,height)
                             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
                             ON CONFLICT(path) DO UPDATE
                             SET dir=?2,name=?3,mtime=?4,rating=?5,capture_at=?6,
                                 width=?7,height=?8,
                                 content_hash = CASE WHEN mtime = excluded.mtime
                                                     THEN content_hash ELSE NULL END",
                            rusqlite::params![path, dir, name, mtime, rating, captured, w, h],
                        )?;
                        added += 1;
                    }
                    tx.commit()?;
                }
            }
            on_progress(dirs, frames_seen);
        }

        // Prune: rows under root whose file vanished from this walk — plus
        // the stored root, in one final transaction.
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let root_like = format!("{}%", root.to_string_lossy());
        let stale: Vec<String> = {
            let mut stmt = tx.prepare("SELECT path FROM frames WHERE path LIKE ?1")?;
            let rows = stmt.query_map([&root_like], |r| r.get::<_, String>(0))?;
            rows.flatten()
                .filter(|p| !found_paths.contains(p.as_str()))
                .collect()
        };
        let removed = stale.len();
        // Second guard, independent of the read_dir-on-root check above: a
        // flaky mount can also return Ok() with a spuriously empty/partial
        // listing while it's still reconnecting, rather than a clean error —
        // that walks "successfully" and finds nothing, same catastrophic
        // result. Refuse to commit a prune that would wipe out (near) an
        // entire substantial root in one go; a legitimate mass-delete is
        // vanishingly rare next to a mount hiccup, and the cost of being
        // wrong here is silent, unrecoverable index loss (the disk files are
        // fine; the ratings/tags/EXIF cache in the index is what's gone).
        const MIN_LIBRARY_FOR_GUARD: usize = 50;
        const CATASTROPHIC_FRACTION: f64 = 0.9;
        if known_under_root >= MIN_LIBRARY_FOR_GUARD
            && removed as f64 >= known_under_root as f64 * CATASTROPHIC_FRACTION
        {
            drop(tx); // uncommitted — rolls back, nothing pruned
            return Err(IndexError::Io(std::io::Error::other(format!(
                "refusing to reindex \u{201c}{}\u{201d}: {removed} of {known_under_root} already-indexed \
                 photos appear to have vanished all at once — this looks like a network mount that just \
                 reconnected, not a real deletion. Nothing was changed; retry once the volume is stable.",
                root.display()
            ))));
        }
        for p in &stale {
            tx.execute("DELETE FROM frames WHERE path=?1", [p])?;
        }
        if update_root {
            // Legacy primary pointer (kept for `root()` and single-root
            // callers) …
            tx.execute(
                "INSERT INTO meta(key,value) VALUES('root',?1)
                 ON CONFLICT(key) DO UPDATE SET value=?1",
                [root.to_string_lossy()],
            )?;
            // … and register it in the multi-root set WITHOUT disturbing any
            // other root (adding a second library no longer evicts the first).
            tx.execute(
                "INSERT OR IGNORE INTO roots(path, added_at)
                 VALUES(?1, COALESCE((SELECT MAX(added_at) FROM roots), 0) + 1)",
                [root.to_string_lossy()],
            )?;
        }
        tx.commit()?;
        drop(conn);

        // Backfill: rows that predate the capture_at column — their mtime
        // skip means the walk above never re-reads them. NULL = never read;
        // 0 = read, no EXIF date (so we don't re-pay the read every scan).
        // Backfill. The walk above skips any file whose mtime is unchanged, so
        // a column added after a library was indexed would otherwise never
        // fill — a rescan reads nothing. This pass is how `capture_at` caught
        // up when it was added, and `width`/`height` ride along in the same
        // header read rather than paying a second one.
        let missing: Vec<String> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT path FROM frames
                 WHERE (capture_at IS NULL OR width IS NULL) AND path LIKE ?1",
            )?;
            let rows = stmt.query_map([&root_like], |r| r.get::<_, String>(0))?;
            rows.flatten().collect()
        };
        for chunk in missing.chunks(64) {
            // In parallel: each of these is one open of a file on an NFS
            // mount, so the chunk's cost is latency, not work, and waiting for
            // them one at a time is waiting for nothing.
            let vals: Vec<(&String, i64, u32, u32)> = chunk
                .par_iter()
                .map(|p| {
                    let (ts, w, h) = reveal_decode::capture_header(Path::new(p));
                    // 0 for "asked, and the file does not say" — the same
                    // sentinel `capture_at` has always used. NULL would mean
                    // "not asked yet" and this pass would read the file again
                    // on every scan, forever.
                    (p, ts.unwrap_or(0), w.unwrap_or(0), h.unwrap_or(0))
                })
                .collect();
            let mut conn = self.conn.lock().unwrap();
            let tx = conn.transaction()?;
            for (p, ts, w, h) in &vals {
                tx.execute(
                    "UPDATE frames SET capture_at=?2, width=?3, height=?4 WHERE path=?1",
                    rusqlite::params![p, ts, w, h],
                )?;
            }
            tx.commit()?;
            on_progress(dirs, frames_seen); // keep the UI's spinner honest
        }

        Ok(ScanStats {
            frames: frames_seen,
            added,
            removed,
            dirs,
            ms: t.elapsed().as_millis(),
        })
    }

    /// Folders holding frames, with counts — Francis's folders are dates,
    /// so lexical order = chronological.
    pub fn dirs(&self) -> Result<Vec<DirRow>, IndexError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT dir, COUNT(*) FROM frames WHERE name NOT LIKE '.%' AND name NOT LIKE '._%' GROUP BY dir ORDER BY dir")?;
        let rows = stmt.query_map([], |r| {
            Ok(DirRow {
                dir: r.get(0)?,
                count: r.get(1)?,
            })
        })?;
        Ok(rows.flatten().collect())
    }

    pub fn frames(&self, dir: &str, min_rating: u8) -> Result<Vec<FrameRow>, IndexError> {
        let conn = self.conn.lock().unwrap();
        // Recursive on purpose (the Swift app's navigate semantics): a folder
        // shows its own frames AND every subfolder's — so the root shows the
        // whole library ("All Library").
        let mut stmt = conn.prepare(
            "SELECT path, name, rating, capture_at, width, height FROM frames
             WHERE (dir=?1 OR dir LIKE ?1 || '/%') AND rating>=?2 AND name NOT LIKE '.%' AND name NOT LIKE '._%'
             ORDER BY COALESCE(capture_at, 0), name",
        )?;
        let rows = stmt.query_map(rusqlite::params![dir, min_rating], |r| {
            Ok(FrameRow {
                path: r.get(0)?,
                name: r.get(1)?,
                rating: r.get::<_, i64>(2)? as u8,
                capture_at: r.get(3)?,
                width: r.get(4)?,
                height: r.get(5)?,
            })
        })?;
        Ok(rows.flatten().collect())
    }

    /// The remembered content hash for a file, if the importer has ever
    /// computed one and the file has not changed since. `None` means "go
    /// read it", never "this file has no hash".
    pub fn content_hash(&self, path: &str) -> Option<String> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT content_hash FROM frames WHERE path=?1",
            [path],
            |r| r.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten()
    }

    /// Remember a file's content hash. Upserts, because the importer learns a
    /// hash at copy time — before any scan has put the row in `frames`.
    pub fn set_content_hash(&self, path: &str, hash: &str) -> Result<(), IndexError> {
        let p = Path::new(path);
        let dir = p.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mtime = std::fs::metadata(p)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO frames(path,dir,name,mtime,rating,content_hash)
             VALUES(?1,?2,?3,?4,0,?5)
             ON CONFLICT(path) DO UPDATE SET content_hash=?5",
            rusqlite::params![path, dir, name, mtime, hash],
        )?;
        Ok(())
    }

    pub fn set_rating(&self, path: &str, rating: u8) -> Result<(), IndexError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE frames SET rating=?2 WHERE path=?1",
            rusqlite::params![path, rating],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A fresh scratch directory under the OS temp dir, unique per call so
    /// parallel tests never collide.
    fn scratch_dir(name: &str) -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("reveal-index-test-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn open_index(dir: &Path) -> Index {
        Index::open(&dir.join("index.sqlite")).unwrap()
    }

    fn write_raw(dir: &Path, name: &str) {
        std::fs::write(dir.join(name), b"fake raw bytes").unwrap();
    }

    #[test]
    fn a_remembered_hash_round_trips_and_survives_a_rescan() {
        let dir = scratch_dir("hash-roundtrip");
        let index = open_index(&dir);
        write_raw(&dir, "a.raf");
        let path = dir.join("a.raf");
        let key = path.to_string_lossy().to_string();

        assert_eq!(index.content_hash(&key), None, "unknown before anyone looks");
        index.set_content_hash(&key, "abc123").unwrap();
        assert_eq!(index.content_hash(&key).as_deref(), Some("abc123"));

        // A rescan must not throw away work the importer paid for.
        index.scan_subtree_with(&dir, |_, _| {}).unwrap();
        assert_eq!(
            index.content_hash(&key).as_deref(),
            Some("abc123"),
            "an unchanged file keeps its hash across a scan"
        );
    }

    /// The dangerous case. A hash that outlives the bytes it describes would
    /// make a genuinely new photo look like one already in the archive — and
    /// Francis formats his cards after importing.
    #[test]
    fn a_rewritten_file_loses_its_remembered_hash() {
        let dir = scratch_dir("hash-staleness");
        let index = open_index(&dir);
        write_raw(&dir, "a.raf");
        let path = dir.join("a.raf");
        let key = path.to_string_lossy().to_string();

        index.scan_subtree_with(&dir, |_, _| {}).unwrap();
        index.set_content_hash(&key, "old-hash").unwrap();
        assert_eq!(index.content_hash(&key).as_deref(), Some("old-hash"));

        // Different bytes, and a different mtime — which is the signal.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(&path, b"completely different bytes").unwrap();
        index.scan_subtree_with(&dir, |_, _| {}).unwrap();

        assert_eq!(
            index.content_hash(&key),
            None,
            "a rewritten file must forget its hash, not keep an empty promise"
        );
    }

    fn frame_count(index: &Index) -> i64 {
        index
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM frames", [], |r| r.get(0))
            .unwrap()
    }

    /// Seed `n` frame rows directly (bypassing a real scan) with paths under
    /// `root` — simulates "the index already knows about a substantial
    /// library here", the precondition for the catastrophic-prune guard.
    fn seed_frames(index: &Index, root: &Path, n: usize) {
        let conn = index.conn.lock().unwrap();
        for i in 0..n {
            let path = root.join(format!("seeded_{i}.raf"));
            conn.execute(
                "INSERT INTO frames(path, dir, name, mtime, rating) VALUES (?1, ?2, ?3, 0, 0)",
                rusqlite::params![
                    path.to_string_lossy(),
                    root.to_string_lossy(),
                    format!("seeded_{i}.raf"),
                ],
            )
            .unwrap();
        }
    }

    #[test]
    fn normal_scan_indexes_and_prunes_small_changes() {
        let root = scratch_dir("normal");
        write_raw(&root, "a.raf");
        write_raw(&root, "b.raf");
        write_raw(&root, "c.raf");
        let index = open_index(&root);

        index.scan(&root).unwrap();
        assert_eq!(frame_count(&index), 3);

        // A single file vanishing is ordinary attrition, not an incident —
        // the guard's minimum-library threshold (50) means this always
        // prunes normally regardless of the 1-in-3 (33%) ratio.
        std::fs::remove_file(root.join("b.raf")).unwrap();
        index.scan(&root).unwrap();
        assert_eq!(frame_count(&index), 2);
    }

    #[test]
    fn root_becoming_unreadable_does_not_wipe_the_index() {
        let root = scratch_dir("vanished-root");
        write_raw(&root, "a.raf");
        write_raw(&root, "b.raf");
        write_raw(&root, "c.raf");
        let index = open_index(&root);

        index.scan(&root).unwrap();
        assert_eq!(frame_count(&index), 3);

        // The real incident: an autofs/NFS mount goes away mid-session.
        // Removing the directory itself reproduces exactly that from the
        // walker's point of view — read_dir(root) fails.
        std::fs::remove_dir_all(&root).unwrap();

        let result = index.scan(&root);
        assert!(result.is_err(), "an unreadable root must error, not silently succeed");
        assert_eq!(
            frame_count(&index),
            3,
            "a root that failed to read must NOT prune the frames it can no longer see"
        );
    }

    #[test]
    fn catastrophic_prune_is_refused_even_when_root_reads_fine() {
        // The harder case the read_dir-on-root check alone can't catch: the
        // mount is mid-reconnect and read_dir(root) returns Ok with a
        // spuriously empty listing rather than an error. Reproduced here by
        // seeding many known frames for a root that is a real, readable, but
        // genuinely empty directory.
        let root = scratch_dir("catastrophic");
        let index = open_index(&root);
        seed_frames(&index, &root, 60);
        assert_eq!(frame_count(&index), 60);

        let result = index.scan(&root);
        assert!(result.is_err(), "wiping ~all of a substantial root in one walk must be refused");
        assert_eq!(frame_count(&index), 60, "the seeded frames must survive the refused prune");
    }

    #[test]
    fn small_libraries_are_exempt_from_the_catastrophic_guard() {
        // Below MIN_LIBRARY_FOR_GUARD, a root legitimately going empty (the
        // user deleted everything in it) must still work — the guard exists
        // for "an established library disappeared at once", not for small
        // folders where that's a perfectly normal thing to do on purpose.
        let root = scratch_dir("small-empty");
        let index = open_index(&root);
        seed_frames(&index, &root, 3);

        index.scan(&root).unwrap();
        assert_eq!(frame_count(&index), 0);
    }
}

#[cfg(test)]
mod dimension_tests {
    use super::*;

    /// Scan a real folder into a throwaway database and check the photos come
    /// back with the size they are SEEN at.
    ///
    /// Reads the photos, writes only to a temp file — the library is never
    /// touched. Ignored by default because it needs a folder of real RAWs:
    ///
    ///   REVEAL_TEST_DIR=/path/to/a/folder \
    ///     cargo test --release -p reveal-index dimensions -- --ignored --nocapture
    /// The case the first version of this test missed: a library that was
    /// already indexed. The scan skips any file whose mtime has not changed,
    /// so a plain rescan reads nothing and a column added later never fills.
    #[test]
    #[ignore = "integration; needs a real folder via REVEAL_TEST_DIR"]
    fn a_rescan_fills_a_size_that_was_missing() {
        let dir = std::env::var("REVEAL_TEST_DIR").unwrap_or_default();
        if dir.is_empty() || !Path::new(&dir).is_dir() {
            return;
        }
        let db = std::env::temp_dir().join(format!("reveal-refill-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&db);
        let index = Index::open(&db).expect("open");
        index.add_root(&dir).expect("add_root");
        index.scan(Path::new(&dir)).expect("first scan");

        // Stand in for a library indexed before the column existed.
        {
            let conn = index.conn.lock().unwrap();
            conn.execute("UPDATE frames SET width=NULL, height=NULL", []).unwrap();
        }
        index.scan(Path::new(&dir)).expect("rescan");

        let sized = index
            .frames(&dir, 0)
            .unwrap()
            .iter()
            .filter(|r| r.width.is_some())
            .count();
        eprintln!("après réindexage : {sized} photos avec dimensions");
        let _ = std::fs::remove_file(&db);
        assert!(sized > 0, "a rescan left the sizes empty");
    }

    #[test]
    #[ignore = "integration; needs a real folder via REVEAL_TEST_DIR"]
    fn a_scan_records_the_size_a_photo_is_seen_at() {
        let dir = std::env::var("REVEAL_TEST_DIR").unwrap_or_default();
        if dir.is_empty() || !Path::new(&dir).is_dir() {
            eprintln!("set REVEAL_TEST_DIR to a folder of RAWs");
            return;
        }
        let db = std::env::temp_dir().join(format!("reveal-dims-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&db);
        let index = Index::open(&db).expect("open");
        index.add_root(&dir).expect("add_root");
        let stats = index.scan(Path::new(&dir)).expect("scan");
        let rows = index.frames(&dir, 0).expect("frames");
        eprintln!("scanné {} fichiers en {} ms", stats.added, stats.ms);

        let sized: Vec<_> = rows.iter().filter(|r| r.width.is_some()).collect();
        let portrait = sized.iter().filter(|r| r.height > r.width).count();
        eprintln!(
            "{}/{} avec dimensions · {} en portrait",
            sized.len(),
            rows.len(),
            portrait
        );
        for r in sized.iter().take(3) {
            eprintln!("   {} {:?}x{:?}", r.name, r.width.unwrap(), r.height.unwrap());
        }

        assert!(!sized.is_empty(), "no photo came back with a size");
        assert!(
            portrait > 0,
            "not one portrait: the sensor rotation is being ignored again"
        );
        let _ = std::fs::remove_file(&db);
    }
}
