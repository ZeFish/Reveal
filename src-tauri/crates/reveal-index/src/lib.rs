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

/// Threads that wait on the file system for `stat`. They hardly use the CPU, so
/// there can be more of them than cores; the reads that follow keep rayon's
/// default pool because those compete for the disks.
static STAT_POOL: std::sync::LazyLock<rayon::ThreadPool> = std::sync::LazyLock::new(|| {
    rayon::ThreadPoolBuilder::new()
        .num_threads(32)
        .thread_name(|i| format!("index-stat-{i}"))
        .build()
        .expect("stat pool")
});

/// `list_folder`, tried again when the file system fails for a reason that may
/// pass. A NAS under many requests at once answers some with a timeout or an
/// I/O error that the next try does not repeat.
fn list_folder_retrying(d: &Path) -> std::io::Result<Listing> {
    let mut attempt = 0;
    loop {
        match list_folder(d) {
            Ok(l) => return Ok(l),
            // Gone, or not ours to read: asking again will not change that.
            Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied) => {
                return Err(e)
            }
            Err(e) if attempt >= 3 => return Err(e),
            Err(_) => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(250 * attempt));
            }
        }
    }
}

/// One folder as the walk needs it.
struct Listing {
    /// Folders to descend into.
    subdirs: Vec<std::path::PathBuf>,
    /// Lower-cased names of the `.xmp` files present, so a photo's sidecar is
    /// known without asking for it.
    xmps: HashSet<String>,
    /// Photos: path, folder, name, modification time.
    batch: Vec<(String, String, String, i64)>,
}

/// List one folder: its subfolders, its sidecars and its photos with their
/// modification times. Run for a whole level of the tree at once.
fn list_folder(d: &Path) -> std::io::Result<Listing> {
    let entries = std::fs::read_dir(d)?;
    // Which sidecars exist, learned from the listing we are already reading.
    // Asking for `NAME.RAF.xmp` on every photo meant an open per file, and most
    // have none: a failed lookup on the NAS is still a round trip — 9.5ms a
    // file, measured cold.
    let mut xmps: HashSet<String> = HashSet::new();
    let mut subdirs = Vec::new();
    // Photos found in this folder: (path, file name). Their modification times
    // are read together below, not one by one.
    let mut candidates: Vec<(std::path::PathBuf, String)> = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        let lower = name.to_lowercase();
        // The listing already says whether an entry is a folder; asking
        // `is_dir()` per entry was a `stat` each, a round trip on a NAS. Only a
        // symlink needs the real answer (it may point at a folder).
        let is_dir = match e.file_type() {
            Ok(t) if t.is_symlink() => p.is_dir(),
            Ok(t) => t.is_dir(),
            Err(_) => p.is_dir(),
        };
        if is_dir {
            if !name.starts_with('.') && !SKIP_DIRS.contains(&lower.as_str()) {
                subdirs.push(p);
            }
            continue;
        }
        if lower.ends_with(".xmp") {
            xmps.insert(lower);
            continue;
        }
        // macOS writes a "._name.raf" AppleDouble sidecar next to every real
        // file on NFS/SMB volumes — same extension, not a photo, and unreadable
        // as one (permanent decode failures).
        if name.starts_with('.') {
            continue;
        }
        let Some(ext) = lower.rsplit('.').next() else { continue };
        if !reveal_decode::RAW_EXTENSIONS.contains(&ext) {
            continue;
        }
        candidates.push((p, name));
    }
    // One `stat` per photo for its modification time, the check that lets a
    // rescan skip everything unchanged. On a NAS each is a round trip whose cost
    // is waiting, not work, so they overlap: serially they were the whole cost
    // of rescanning a library that had not changed.
    let dir_string = d.to_string_lossy().into_owned();
    let batch = candidates
        .into_par_iter()
        .map(|(p, name)| {
            let mtime = std::fs::metadata(&p)
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            (p.to_string_lossy().into_owned(), dir_string.clone(), name, mtime)
        })
        .collect();
    Ok(Listing { subdirs, xmps, batch })
}

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("sqlite: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// A library may not sit inside another, nor contain one. The text is
    /// written to be shown to the person as it is.
    #[error("{0}")]
    Nested(String),
}

/// A path in one spelling. macOS reaches the data volume both directly and
/// through firmlinks (`/System/Volumes/Data/mnt/x` is `/mnt/x`); only the
/// second is what a person types.
fn normal_form(path: &str) -> &str {
    path.strip_prefix("/System/Volumes/Data")
        .filter(|rest| rest.starts_with('/'))
        .unwrap_or(path)
        .trim_end_matches('/')
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
        //
        // ONCE. It used to run on every open and never cleared `meta.root`,
        // so a library you removed came back at the next launch — Francis
        // removed "Capture" twice and it returned twice (2026-09-23). The key
        // is consumed here; `roots` is the only truth from now on.
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
            let _ = conn.execute("DELETE FROM meta WHERE key='root'", []);
        }
        // Libraries do not nest. A database from before that rule can hold one
        // inside another ("Capture" inside "ffp-production"). The inner one
        // adds nothing: the outer lists every photo beneath it. It goes from
        // the list; no photo row is touched.
        {
            let roots: Vec<String> = {
                let mut stmt = conn.prepare("SELECT path FROM roots")?;
                let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
                rows.flatten().collect()
            };
            for inner in &roots {
                let inner_n = normal_form(inner);
                let held = roots.iter().any(|outer| {
                    outer != inner && inner_n.starts_with(&format!("{}/", normal_form(outer)))
                });
                if held {
                    conn.execute("DELETE FROM roots WHERE path=?1", [inner])?;
                }
            }
        }
        Ok(Self {
            conn: Mutex::new(conn),
        })
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
        // Only the photos no remaining root still covers. Roots can nest —
        // "Capture" lived inside "ffp-production" — and deleting everything
        // under the removed path threw away rows the parent still owned:
        // Francis lost indexed photos from ffp-production each time he
        // removed Capture (2026-09-23).
        //
        // Prefixes compared with `substr`, not LIKE: `_` and `%` are
        // wildcards to LIKE and perfectly ordinary in a folder name.
        let n = conn.execute(
            "DELETE FROM frames
             WHERE (path = ?1 OR substr(path, 1, length(?1) + 1) = ?1 || '/')
               AND NOT EXISTS (
                 SELECT 1 FROM roots r
                 WHERE frames.path = r.path
                    OR substr(frames.path, 1, length(r.path) + 1) = r.path || '/'
               )",
            [path],
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

    /// The library that already holds `path`: a root equal to it or above it.
    /// The longest match wins, if roots nest from before nesting was forbidden.
    ///
    /// Compared in a normal form, because macOS has two spellings of one place:
    /// `/mnt/x` and `/System/Volumes/Data/mnt/x`. A folder picker returns one and
    /// an older preference holds the other; they are the same folder.
    pub fn library_covering(&self, path: &str) -> Result<Option<String>, IndexError> {
        let p = normal_form(path);
        Ok(self
            .roots()?
            .into_iter()
            .filter(|r| {
                let r = normal_form(r);
                p == r || p.starts_with(&format!("{r}/"))
            })
            .max_by_key(|r| r.len()))
    }

    /// Whether `path` may become a library of its own. Libraries do not nest:
    /// a folder already inside one is already in the library (it shows there),
    /// and a folder that contains one would swallow it.
    ///
    /// An existing root is always fine; that is a rescan, not an addition.
    pub fn check_can_add_root(&self, path: &str) -> Result<(), IndexError> {
        let roots = self.roots()?;
        let p = normal_form(path);
        if roots.iter().any(|r| normal_form(r) == p) {
            return Ok(());
        }
        let name = |s: &str| s.rsplit('/').find(|x| !x.is_empty()).unwrap_or(s).to_string();
        if let Some(outer) = roots.iter().find(|r| {
            let r = normal_form(r);
            p.starts_with(&format!("{r}/"))
        }) {
            return Err(IndexError::Nested(format!(
                "\u{201c}{}\u{201d} is already part of the library \u{201c}{}\u{201d}, where its photos show. Libraries can\u{2019}t be nested, so it isn\u{2019}t added again.",
                name(path),
                name(outer)
            )));
        }
        if let Some(inner) = roots.iter().find(|r| normal_form(r).starts_with(&format!("{p}/"))) {
            return Err(IndexError::Nested(format!(
                "\u{201c}{}\u{201d} contains the library \u{201c}{}\u{201d}. Libraries can\u{2019}t be nested: remove \u{201c}{}\u{201d} first, then add this folder.",
                name(path),
                name(inner),
                name(inner)
            )));
        }
        Ok(())
    }

    /// Whether a folder may be moved under `dest_parent`. A folder that is a
    /// library, or holds one, cannot go inside another library: that would nest
    /// them, and libraries do not nest.
    pub fn check_can_move(&self, src: &str, dest_parent: &str) -> Result<(), IndexError> {
        let s = normal_form(src);
        let holds_library = self.roots()?.iter().any(|r| {
            let r = normal_form(r);
            r == s || r.starts_with(&format!("{s}/"))
        });
        if !holds_library {
            return Ok(());
        }
        if let Some(outer) = self.library_covering(dest_parent)? {
            let name = |p: &str| p.rsplit('/').find(|x| !x.is_empty()).unwrap_or(p).to_string();
            return Err(IndexError::Nested(format!(
                "\u{201c}{}\u{201d} is, or contains, a library, and \u{201c}{}\u{201d} is already one. Libraries can\u{2019}t be nested, so it isn\u{2019}t moved.",
                name(src),
                name(&outer)
            )));
        }
        Ok(())
    }

    /// A folder was renamed or moved on disk: carry the index with it. Library
    /// roots at or under `old` follow to `new`, and so do the photos' rows, so
    /// their ratings and remembered hashes survive. Rows that end up outside
    /// every library (the folder was moved out of one) are forgotten; the files
    /// are untouched.
    pub fn relocate(&self, old: &str, new: &str) -> Result<(), IndexError> {
        let old = old.trim_end_matches('/');
        let new = new.trim_end_matches('/');
        // The same place has two spellings on macOS; rows may use either.
        let mut pairs = vec![(old.to_string(), new.to_string())];
        if old.starts_with('/') && !old.starts_with("/System/Volumes/Data/") {
            pairs.push((format!("/System/Volumes/Data{old}"), format!("/System/Volumes/Data{new}")));
        }
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        for (o, n) in &pairs {
            tx.execute(
                "UPDATE OR IGNORE roots SET path = ?2 || substr(path, length(?1) + 1)
                 WHERE path = ?1 OR substr(path, 1, length(?1) + 1) = ?1 || '/'",
                [o, n],
            )?;
            tx.execute(
                "UPDATE frames SET path = ?2 || substr(path, length(?1) + 1),
                                   dir = ?2 || substr(dir, length(?1) + 1)
                 WHERE substr(path, 1, length(?1) + 1) = ?1 || '/'",
                [o, n],
            )?;
            tx.execute(
                "DELETE FROM frames
                 WHERE substr(path, 1, length(?1) + 1) = ?1 || '/'
                   AND NOT EXISTS (
                     SELECT 1 FROM roots r
                     WHERE frames.path = r.path
                        OR substr(frames.path, 1, length(r.path) + 1) = r.path || '/'
                   )",
                [n],
            )?;
        }
        tx.commit()?;
        Ok(())
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
        // A scan that registers its path as a library is how a library is added.
        // Refuse before walking anything if that would nest one in another.
        if update_root {
            self.check_can_add_root(&root.to_string_lossy())?;
        }
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
        let mut unreadable: Vec<String> = Vec::new();
        // The walk goes a level at a time. Every folder of a level is listed
        // together: a cold NAS makes each listing a wait, and they overlap.
        let mut wave: Vec<std::path::PathBuf> = std::mem::take(&mut stack);
        while !wave.is_empty() {
            let listings: Vec<(std::path::PathBuf, std::io::Result<Listing>)> = STAT_POOL.install(|| {
                wave.par_iter().map(|d| (d.clone(), list_folder_retrying(d))).collect()
            });
            wave = Vec::new();
            for (d, listing) in listings {
                let listing = match listing {
                    Ok(l) => l,
                    Err(e) => {
                        if first {
                            return Err(IndexError::Io(e));
                        }
                        first = false;
                        // A folder that cannot be read right now is not a folder
                        // that is gone. Its photos stay in the index: pruning
                        // them would lose their ratings over a timeout. (A
                        // folder that no longer exists is pruned as before.)
                        if e.kind() != std::io::ErrorKind::NotFound {
                            eprintln!("index: could not read {}: {e}; keeping what it held", d.display());
                            unreadable.push(format!("{}/", d.to_string_lossy().trim_end_matches('/')));
                        }
                        continue;
                    }
                };
                first = false;
                dirs += 1;
                let _ = &d;
                wave.extend(listing.subdirs);
                let xmps = listing.xmps;
                let batch = listing.batch;

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
                // Up to two reads per file, both of them round trips to the NAS:
                // the XMP sidecar for the rating (only when the listing above
                // says there is one), and the RAW's header for the date and
                // the size.
                //
                // What it costs, measured cold on this library (2026-09-23):
                // the first touch of a RAW takes ~165ms whether 64 KB or the
                // whole header is read — it is the NAS finding the file, not
                // the bytes. So the floor is one touch per new file, which is
                // what this does, and the only lever left is having several in
                // flight: one at a time ran 159–209ms a file, rayon's default
                // (8 here) 67–97ms. More does NOT help — 32 in flight went to
                // 140ms and 64 to 176ms, the disks fighting over their seeks.
                //
                // (libraw cannot be handed a prefix instead: `open_buffer`
                // validates its offsets against the buffer and refused 26 of
                // 30 files even at 1 MB.)
                rows = to_read
                    .into_par_iter()
                    .map(|(path, dir, name, mtime)| {
                        let has_sidecar = xmps.contains(&format!("{}.xmp", name.to_lowercase()));
                        let rating = if has_sidecar {
                            reveal_meta::read(Path::new(&path))
                                .ok()
                                .flatten()
                                .and_then(|s| s.rating)
                                .unwrap_or(0)
                        } else {
                            0
                        };
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
                .filter(|p| !unreadable.iter().any(|d| p.starts_with(d.as_str())))
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
            // Register it in the multi-root set WITHOUT disturbing any other
            // root (adding a second library no longer evicts the first). The
            // legacy `meta.root` pointer is not written any more: nothing read
            // it but the migration, which is how a removed library kept
            // resurrecting itself.
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

    /// Capture times already read for the frames under `dir` (unix seconds,
    /// omitting those with no date). Lets a caller skip opening files whose date
    /// the index holds already; over a network mount that is most of the cost.
    pub fn capture_times_under(&self, dir: &str) -> Result<std::collections::HashMap<String, i64>, IndexError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT path, capture_at FROM frames
             WHERE capture_at > 0 AND (dir = ?1 OR substr(dir, 1, length(?1) + 1) = ?1 || '/')",
        )?;
        let rows = stmt.query_map([dir], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        Ok(rows.flatten().collect())
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

    fn insert_frame(index: &Index, path: &str, rating: i64) {
        let dir = path.rsplit_once('/').unwrap().0;
        let name = path.rsplit_once('/').unwrap().1;
        let conn = index.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO frames(path, dir, name, mtime, rating) VALUES(?1, ?2, ?3, 1, ?4)",
            rusqlite::params![path, dir, name, rating],
        )
        .unwrap();
    }

    fn rating_of(index: &Index, path: &str) -> Option<i64> {
        let conn = index.conn.lock().unwrap();
        conn.query_row("SELECT rating FROM frames WHERE path=?1", [path], |r| r.get(0)).ok()
    }

    #[test]
    fn renaming_a_library_folder_carries_the_root_and_the_photos_with_it() {
        let dir = scratch_dir("relocate-rename");
        let index = open_index(&dir);
        index.add_root("/p/Photos").unwrap();
        insert_frame(&index, "/p/Photos/2026/a.raf", 4);
        insert_frame(&index, "/p/Other/b.raf", 1);

        index.relocate("/p/Photos", "/p/Archive").unwrap();

        assert_eq!(index.roots().unwrap(), vec!["/p/Archive".to_string()]);
        assert_eq!(rating_of(&index, "/p/Archive/2026/a.raf"), Some(4), "rating survives");
        assert_eq!(rating_of(&index, "/p/Photos/2026/a.raf"), None);
        assert_eq!(index.dirs().unwrap().iter().filter(|d| d.dir == "/p/Archive/2026").count(), 1);
    }

    #[test]
    fn a_folder_inside_a_library_moves_without_touching_its_siblings() {
        let dir = scratch_dir("relocate-inside");
        let index = open_index(&dir);
        index.add_root("/p/Lib").unwrap();
        insert_frame(&index, "/p/Lib/a/1.raf", 2);
        insert_frame(&index, "/p/Lib/ab/2.raf", 3);
        index.relocate("/p/Lib/a", "/p/Lib/z/a").unwrap();
        assert_eq!(rating_of(&index, "/p/Lib/z/a/1.raf"), Some(2));
        assert_eq!(rating_of(&index, "/p/Lib/ab/2.raf"), Some(3), "a longer name sharing the prefix is not a child");
        assert_eq!(index.roots().unwrap(), vec!["/p/Lib".to_string()]);
    }

    #[test]
    fn moving_a_folder_out_of_every_library_forgets_its_rows_only() {
        let dir = scratch_dir("relocate-out");
        let index = open_index(&dir);
        index.add_root("/p/Lib").unwrap();
        insert_frame(&index, "/p/Lib/a/1.raf", 2);
        insert_frame(&index, "/p/Lib/keep.raf", 5);
        index.relocate("/p/Lib/a", "/elsewhere/a").unwrap();
        assert_eq!(rating_of(&index, "/elsewhere/a/1.raf"), None);
        assert_eq!(rating_of(&index, "/p/Lib/keep.raf"), Some(5));
    }

    #[test]
    fn a_library_cannot_be_moved_into_another_and_a_plain_folder_can() {
        let dir = scratch_dir("move-check");
        let index = open_index(&dir);
        index.add_root("/p/Lib").unwrap();
        index.add_root("/q/Other").unwrap();
        index.add_root("/q/Parent").unwrap_or(());
        // A library into a library: nesting.
        assert!(matches!(index.check_can_move("/q/Other", "/p/Lib/sub"), Err(IndexError::Nested(_))));
        // A folder that contains a library, into a library: also nesting.
        let index2 = open_index(&scratch_dir("move-check-2"));
        index2.add_root("/q/Group/Inner").unwrap();
        index2.add_root("/p/Lib").unwrap();
        assert!(matches!(index2.check_can_move("/q/Group", "/p/Lib"), Err(IndexError::Nested(_))));
        // A plain folder in a library, anywhere: fine. A library out of all libraries: fine.
        assert!(index.check_can_move("/p/Lib/a", "/p/Lib/b").is_ok());
        assert!(index.check_can_move("/q/Other", "/somewhere/else").is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn a_folder_that_cannot_be_read_keeps_its_photos_and_a_deleted_one_loses_them() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch_dir("unreadable");
        let index = open_index(&dir);
        for sub in ["kept", "gone", "fine"] {
            std::fs::create_dir_all(dir.join(sub)).unwrap();
            write_raw(&dir.join(sub), "a.raf");
        }
        index.add_root(&dir.to_string_lossy()).unwrap();
        index.scan(&dir).unwrap();
        let count = |index: &Index| index.dirs().unwrap().iter().map(|d| d.count as usize).sum::<usize>();
        assert_eq!(count(&index), 3);

        std::fs::set_permissions(dir.join("kept"), std::fs::Permissions::from_mode(0o000)).unwrap();
        std::fs::remove_dir_all(dir.join("gone")).unwrap();
        let result = index.scan(&dir);
        std::fs::set_permissions(dir.join("kept"), std::fs::Permissions::from_mode(0o755)).unwrap();
        result.unwrap();

        assert_eq!(count(&index), 2, "the unreadable folder's photo stays, the deleted one's goes");
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
        eprintln!("after reindexing: {sized} photos with dimensions");
        let _ = std::fs::remove_file(&db);
        assert!(sized > 0, "a rescan left the sizes empty");
    }

    /// Ratings now come only from sidecars the directory listing says exist,
    /// instead of trying to open one next to every photo. If that lookup ever
    /// missed a sidecar, its rating would silently read as 0 — so compare
    /// every row against a direct read.
    #[test]
    #[ignore = "integration; needs a real folder via REVEAL_TEST_DIR"]
    fn ratings_survive_the_sidecar_shortcut() {
        let dir = std::env::var("REVEAL_TEST_DIR").unwrap_or_default();
        if dir.is_empty() || !Path::new(&dir).is_dir() {
            return;
        }
        let db = std::env::temp_dir().join(format!("reveal-ratings-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&db);
        let index = Index::open(&db).expect("open");
        index.add_root(&dir).expect("add_root");
        index.scan(Path::new(&dir)).expect("scan");
        let rows = index.frames(&dir, 0).unwrap();
        let (mut with_sidecar, mut rated, mut wrong) = (0, 0, 0);
        for r in &rows {
            let direct = reveal_meta::read(Path::new(&r.path)).ok().flatten();
            if direct.is_some() {
                with_sidecar += 1;
            }
            let want = direct.and_then(|s| s.rating).unwrap_or(0);
            if want > 0 {
                rated += 1;
            }
            if want != r.rating {
                wrong += 1;
                eprintln!("   mismatch: {} index={} sidecar={}", r.name, r.rating, want);
            }
        }
        eprintln!("{} photos · {with_sidecar} with sidecar · {rated} rated · {wrong} mismatches", rows.len());
        let _ = std::fs::remove_file(&db);
        assert_eq!(wrong, 0);
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
        eprintln!("scanned {} files in {} ms", stats.added, stats.ms);

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

#[cfg(test)]
mod root_tests {
    use super::*;

    fn temp_index(tag: &str) -> (Index, std::path::PathBuf) {
        let db = std::env::temp_dir().join(format!("reveal-roots-{tag}-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&db);
        (Index::open(&db).expect("open"), db)
    }

    fn put(index: &Index, path: &str) {
        let conn = index.conn.lock().unwrap();
        let dir = &path[..path.rfind('/').unwrap()];
        conn.execute(
            "INSERT INTO frames(path,dir,name,mtime,rating) VALUES(?1,?2,?3,0,0)",
            rusqlite::params![path, dir, &path[dir.len() + 1..]],
        )
        .unwrap();
    }

    fn count(index: &Index) -> i64 {
        index.conn.lock().unwrap().query_row("SELECT count(*) FROM frames", [], |r| r.get(0)).unwrap()
    }

    /// The exact shape Francis hit: a library nested inside another. Removing
    /// the inner one must not forget photos the outer one still covers.
    #[test]
    fn removing_a_nested_root_keeps_what_the_parent_covers() {
        let (index, db) = temp_index("nested");
        index.add_root("/nas/ffp").unwrap();
        index.add_root("/nas/ffp/Personelle/Capture").unwrap();
        put(&index, "/nas/ffp/Personelle/Capture/2026/a.RAF");
        put(&index, "/nas/ffp/Other/b.RAF");

        let removed = index.remove_root("/nas/ffp/Personelle/Capture").unwrap();
        assert_eq!(removed, 0);
        assert_eq!(count(&index), 2);
        assert_eq!(index.roots().unwrap(), vec!["/nas/ffp".to_string()]);
        let _ = std::fs::remove_file(&db);
    }

    /// The other direction: removing the parent forgets what only it covered,
    /// and keeps what the nested root still owns.
    #[test]
    fn removing_the_parent_keeps_the_nested_root_s_photos() {
        let (index, db) = temp_index("parent");
        index.add_root("/nas/ffp").unwrap();
        index.add_root("/nas/ffp/Capture").unwrap();
        put(&index, "/nas/ffp/Capture/a.RAF");
        put(&index, "/nas/ffp/Other/b.RAF");

        assert_eq!(index.remove_root("/nas/ffp").unwrap(), 1);
        assert_eq!(count(&index), 1);
        let _ = std::fs::remove_file(&db);
    }

    /// A sibling whose name merely starts with the same characters is not
    /// inside the root — and `_` must not act as a wildcard.
    #[test]
    fn a_prefix_is_not_containment() {
        let (index, db) = temp_index("prefix");
        index.add_root("/nas/ffp").unwrap();
        index.add_root("/nas/ffp_2").unwrap();
        put(&index, "/nas/ffp/a.RAF");
        put(&index, "/nas/ffp-production/b.RAF");
        put(&index, "/nas/ffpX2/c.RAF");

        assert_eq!(index.remove_root("/nas/ffp").unwrap(), 1);
        assert_eq!(count(&index), 2);
        let _ = std::fs::remove_file(&db);
    }

    /// A removed library stays removed across a restart. The legacy
    /// `meta.root` pointer used to re-seed it on every open.
    #[test]
    fn a_removed_root_stays_removed_after_reopening() {
        let (index, db) = temp_index("reopen");
        {
            let conn = index.conn.lock().unwrap();
            conn.execute("INSERT INTO meta(key,value) VALUES('root','/nas/ffp/Capture')", []).unwrap();
        }
        drop(index);
        let index = Index::open(&db).unwrap(); // migration seeds it, once
        assert_eq!(index.roots().unwrap(), vec!["/nas/ffp/Capture".to_string()]);

        index.remove_root("/nas/ffp/Capture").unwrap();
        drop(index);
        let index = Index::open(&db).unwrap(); // the restart
        assert!(index.roots().unwrap().is_empty(), "the removed library came back");
        let _ = std::fs::remove_file(&db);
    }

    // ---- libraries do not nest ---------------------------------------------

    #[test]
    fn a_library_cannot_be_added_inside_another() {
        let (index, db) = temp_index("no-nest-inside");
        index.add_root("/nas/ffp").unwrap();
        let err = index.check_can_add_root("/nas/ffp/Personelle/Capture").unwrap_err();
        assert!(err.to_string().contains("already part of the library"), "{err}");
        assert!(err.to_string().contains("ffp"));
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn a_library_cannot_swallow_another() {
        let (index, db) = temp_index("no-nest-contains");
        index.add_root("/nas/ffp/Capture").unwrap();
        let err = index.check_can_add_root("/nas/ffp").unwrap_err();
        assert!(err.to_string().contains("contains the library"), "{err}");
        assert!(err.to_string().contains("remove"));
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn rescanning_a_library_and_adding_a_sibling_are_fine() {
        let (index, db) = temp_index("nest-ok");
        index.add_root("/nas/ffp").unwrap();
        assert!(index.check_can_add_root("/nas/ffp").is_ok(), "an existing root is a rescan");
        assert!(index.check_can_add_root("/nas/ffp-2").is_ok(), "a shared prefix is not containment");
        assert!(index.check_can_add_root("/nas/other").is_ok());
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn the_two_spellings_of_one_mac_folder_are_one_folder() {
        let (index, db) = temp_index("spellings");
        index.add_root("/System/Volumes/Data/mnt/ffp").unwrap();
        assert!(index.check_can_add_root("/mnt/ffp/Capture").is_err());
        assert!(index.check_can_add_root("/mnt/ffp").is_ok());
        assert_eq!(
            index.library_covering("/mnt/ffp/Capture/2026").unwrap().as_deref(),
            Some("/System/Volumes/Data/mnt/ffp")
        );
        assert_eq!(index.library_covering("/mnt/ffp-2/x").unwrap(), None);
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn a_scan_that_would_nest_refuses_before_walking() {
        let (index, db) = temp_index("scan-refuses");
        let outer = std::env::temp_dir().join(format!("reveal-nest-{}", std::process::id()));
        let inner = outer.join("inner");
        std::fs::create_dir_all(&inner).unwrap();
        index.add_root(&outer.to_string_lossy()).unwrap();
        let err = index.scan(&inner).err().expect("nesting must be refused");
        assert!(matches!(err, IndexError::Nested(_)));
        assert_eq!(index.roots().unwrap().len(), 1, "no second library was registered");
        let _ = std::fs::remove_dir_all(&outer);
        let _ = std::fs::remove_file(&db);
    }

    #[test]
    fn opening_a_database_that_nests_libraries_flattens_it_without_losing_a_photo() {
        let (index, db) = temp_index("flatten");
        index.add_root("/nas/ffp").unwrap();
        index.add_root("/nas/ffp/Personelle/Capture").unwrap();
        index.add_root("/nas/elsewhere").unwrap();
        put(&index, "/nas/ffp/Personelle/Capture/2026/a.RAF");
        put(&index, "/nas/ffp/Other/b.RAF");
        put(&index, "/nas/elsewhere/c.RAF");
        drop(index);

        let reopened = Index::open(&db).unwrap();
        assert_eq!(
            reopened.roots().unwrap(),
            vec!["/nas/ffp".to_string(), "/nas/elsewhere".to_string()]
        );
        assert_eq!(count(&reopened), 3, "no photo was touched");
        let _ = std::fs::remove_file(&db);
    }
}
