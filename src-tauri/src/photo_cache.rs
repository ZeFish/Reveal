//! Bounded PhotoKit working-copy storage. Only generated cache entries are
//! managed here; durable XMP edits live in a different application-data tree.
use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

pub const GIB: u64 = 1024 * 1024 * 1024;
pub const DEFAULT_LIMIT: u64 = 4 * GIB;

#[derive(serde::Serialize)]
pub struct CacheStatus {
    pub size_bytes: u64,
    pub limit_bytes: u64,
    pub in_use_bytes: u64,
}

#[derive(Debug, serde::Serialize)]
pub struct Cleanup {
    pub removed_bytes: u64,
    pub remaining_bytes: u64,
    pub protected_bytes: u64,
}

#[derive(Default)]
struct State {
    active: HashMap<PathBuf, usize>,
}

pub struct PhotoCache {
    root: PathBuf,
    limit: AtomicU64,
    state: Mutex<State>,
    scheduled: AtomicBool,
    automatic: bool,
    on_error: Box<dyn Fn(&str) + Send + Sync>,
}

struct Entry {
    path: PathBuf,
    files: Vec<PathBuf>,
    bytes: u64,
    used: SystemTime,
}

/// Hold this through download AND decoding/reading, not just path resolution.
pub struct CacheLease {
    cache: Arc<PhotoCache>,
    path: PathBuf,
}

impl CacheLease {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CacheLease {
    fn drop(&mut self) {
        match self.cache.state.lock() {
            Ok(mut state) => {
                if let Some(count) = state.active.get_mut(&self.path) {
                    *count -= 1;
                    if *count == 0 {
                        state.active.remove(&self.path);
                    }
                }
            }
            Err(error) => (self.cache.on_error)(&format!("Cache lease release failed: {error}")),
        }
        self.cache.schedule_trim();
    }
}

impl PhotoCache {
    pub fn new(
        root: PathBuf,
        limit: u64,
        on_error: impl Fn(&str) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        reject_symlink(&root)?;
        fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        Ok(Self {
            root: root.canonicalize().map_err(|e| e.to_string())?,
            limit: AtomicU64::new(limit),
            state: Mutex::new(State::default()),
            scheduled: AtomicBool::new(false),
            automatic: true,
            on_error: Box::new(on_error),
        })
    }

    pub fn lease(self: &Arc<Self>, asset: &str, group: &str) -> Result<CacheLease, String> {
        if !valid_asset(asset) || !valid_group(group) {
            return Err("Invalid Apple Photos cache entry".to_string());
        }
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        reject_symlink(&self.root)?;
        let parent = self.root.join(asset);
        reject_symlink(&parent)?;
        let path = parent.join(group);
        reject_symlink(&path)?;
        fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        let marker = path.join(".last-used");
        reject_symlink(&marker)?;
        File::options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(marker)
            .and_then(|file| file.set_modified(SystemTime::now()))
            .map_err(|e| format!("Could not update photo-cache access time: {e}"))?;
        *state.active.entry(path.clone()).or_default() += 1;
        Ok(CacheLease {
            cache: self.clone(),
            path,
        })
    }

    pub fn set_limit(self: &Arc<Self>, limit: u64) {
        self.limit.store(limit, Ordering::Release);
        self.schedule_trim();
    }

    pub fn status(&self) -> Result<CacheStatus, String> {
        let state = self.state.lock().map_err(|e| e.to_string())?;
        let entries = self.entries()?;
        Ok(CacheStatus {
            size_bytes: entries.iter().map(|entry| entry.bytes).sum(),
            limit_bytes: self.limit.load(Ordering::Acquire),
            in_use_bytes: entries
                .iter()
                .filter(|entry| state.active.contains_key(&entry.path))
                .map(|entry| entry.bytes)
                .sum(),
        })
    }

    pub fn clear(&self) -> Result<Cleanup, String> {
        self.cleanup(0)
    }

    pub fn trim(&self) -> Result<Cleanup, String> {
        self.cleanup(self.limit.load(Ordering::Acquire))
    }

    fn cleanup(&self, target: u64) -> Result<Cleanup, String> {
        // A lease cannot start between choosing an eviction and removing its
        // files. Existing leases stay protected without blocking their work.
        let state = self.state.lock().map_err(|e| e.to_string())?;
        let mut entries = self.entries()?;
        let mut remaining: u64 = entries.iter().map(|entry| entry.bytes).sum();
        let protected = entries
            .iter()
            .filter(|entry| state.active.contains_key(&entry.path))
            .map(|entry| entry.bytes)
            .sum();
        let mut removed = 0;
        entries.sort_by_key(|entry| entry.used);
        for entry in entries {
            if target > 0 && remaining <= target {
                break;
            }
            if state.active.contains_key(&entry.path) {
                continue;
            }
            for file in &entry.files {
                fs::remove_file(file).map_err(|e| {
                    format!("Could not remove cached photo {}: {e}", file.display())
                })?;
            }
            fs::remove_dir(&entry.path).map_err(|e| e.to_string())?;
            if let Some(parent) = entry.path.parent() {
                match fs::remove_dir(parent) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => {}
                    Err(error) => return Err(error.to_string()),
                }
            }
            removed += entry.bytes;
            remaining -= entry.bytes;
        }
        Ok(Cleanup {
            removed_bytes: removed,
            remaining_bytes: remaining,
            protected_bytes: protected,
        })
    }

    fn entries(&self) -> Result<Vec<Entry>, String> {
        reject_symlink(&self.root)?;
        let mut entries = Vec::new();
        let assets = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(entries),
            Err(error) => return Err(error.to_string()),
        };
        for asset in assets {
            let asset = asset.map_err(|e| e.to_string())?;
            if !valid_asset(&asset.file_name().to_string_lossy()) {
                continue;
            }
            reject_symlink(&asset.path())?;
            if !asset.file_type().map_err(|e| e.to_string())?.is_dir() {
                continue;
            }
            for group in fs::read_dir(asset.path()).map_err(|e| e.to_string())? {
                let group = group.map_err(|e| e.to_string())?;
                if !valid_group(&group.file_name().to_string_lossy()) {
                    continue;
                }
                reject_symlink(&group.path())?;
                if !group.file_type().map_err(|e| e.to_string())?.is_dir() {
                    continue;
                }
                let mut entry = Entry {
                    path: group.path(),
                    files: Vec::new(),
                    bytes: 0,
                    used: SystemTime::UNIX_EPOCH,
                };
                let mut accessed = None;
                for file in fs::read_dir(&entry.path).map_err(|e| e.to_string())? {
                    let file = file.map_err(|e| e.to_string())?;
                    reject_symlink(&file.path())?;
                    let metadata = match file.metadata() {
                        Ok(metadata) => metadata,
                        // Atomic PhotoKit writes may rename a temporary file
                        // between readdir and stat while its lease is active.
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                        Err(error) => return Err(error.to_string()),
                    };
                    if !metadata.is_file() {
                        return Err(format!(
                            "Unexpected directory in photo cache: {}",
                            file.path().display()
                        ));
                    }
                    let modified = metadata.modified().map_err(|e| e.to_string())?;
                    if file.file_name() == ".last-used" {
                        accessed = Some(modified);
                    } else {
                        entry.used = entry.used.max(modified);
                    }
                    entry.bytes += metadata.len();
                    entry.files.push(file.path());
                }
                entry.used = accessed.unwrap_or(entry.used);
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    pub fn schedule_trim(self: &Arc<Self>) {
        if !self.automatic || self.scheduled.swap(true, Ordering::AcqRel) {
            return;
        }
        let cache = self.clone();
        std::thread::spawn(move || {
            // Coalesce a grid's burst of thumbnail requests into one scan.
            std::thread::sleep(Duration::from_millis(500));
            // Clear before scanning: a lease released during maintenance must
            // be able to schedule the next pass if it kept us over the limit.
            cache.scheduled.store(false, Ordering::Release);
            if let Err(error) = cache.trim() {
                (cache.on_error)(&error);
            }
        });
    }
}

fn reject_symlink(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "Refusing to follow a link in the photo cache: {}",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn valid_asset(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1024
        && value.len() % 2 == 0
        && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_group(value: &str) -> bool {
    value == "previews"
        || value.strip_prefix("source-").is_some_and(|revision| {
            revision.len() == 16 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache(root: &Path, limit: u64) -> Arc<PhotoCache> {
        let mut cache =
            PhotoCache::new(root.to_path_buf(), limit, |error| panic!("{error}")).unwrap();
        cache.automatic = false;
        Arc::new(cache)
    }

    fn entry(cache: &Arc<PhotoCache>, asset: &str, bytes: usize, age: u64) -> PathBuf {
        let lease = cache.lease(asset, "source-0000000000000001").unwrap();
        fs::write(lease.path().join("working.tiff"), vec![0; bytes]).unwrap();
        let path = lease.path().to_path_buf();
        drop(lease);
        File::options()
            .write(true)
            .open(path.join(".last-used"))
            .unwrap()
            .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(age))
            .unwrap();
        path
    }

    #[test]
    fn evicts_least_recently_used_whole_entries() {
        let temp = tempfile::tempdir().unwrap();
        let cache = cache(temp.path(), 20);
        let oldest = entry(&cache, "aa", 10, 1);
        let middle = entry(&cache, "bb", 10, 2);
        let recent = entry(&cache, "cc", 10, 3);
        let result = cache.trim().unwrap();
        assert_eq!(result.removed_bytes, 10);
        assert_eq!(result.remaining_bytes, 20);
        assert!(!oldest.exists());
        assert!(middle.exists() && recent.exists());
    }

    #[test]
    fn clear_and_trim_preserve_busy_files_and_durable_edits() {
        let temp = tempfile::tempdir().unwrap();
        let edits = temp.path().join("edits");
        fs::create_dir(&edits).unwrap();
        fs::write(edits.join("photo.xmp"), b"settings and ratings").unwrap();
        let cache = cache(&temp.path().join("cache"), 5);
        let busy = cache.lease("aa", "source-0000000000000001").unwrap();
        fs::write(busy.path().join("working.tiff"), [0; 10]).unwrap();
        let second_reader = cache.lease("aa", "source-0000000000000001").unwrap();
        let idle = entry(&cache, "bb", 10, 1);
        let result = cache.clear().unwrap();
        assert_eq!(result.removed_bytes, 10);
        assert_eq!(result.remaining_bytes, 10);
        assert_eq!(result.protected_bytes, 10);
        assert!(busy.path().join("working.tiff").exists());
        assert!(!idle.exists());
        drop(busy);
        assert_eq!(cache.trim().unwrap().protected_bytes, 10);
        drop(second_reader);
        assert_eq!(cache.trim().unwrap().remaining_bytes, 0);
        assert_eq!(
            fs::read(edits.join("photo.xmp")).unwrap(),
            b"settings and ratings"
        );
    }

    #[test]
    fn reading_an_entry_refreshes_lru_without_changing_media() {
        let temp = tempfile::tempdir().unwrap();
        let cache = cache(temp.path(), 10);
        let old = entry(&cache, "aa", 10, 1);
        let newer = entry(&cache, "bb", 10, 2);
        let media = fs::metadata(old.join("working.tiff"))
            .unwrap()
            .modified()
            .unwrap();
        drop(cache.lease("aa", "source-0000000000000001").unwrap());
        cache.trim().unwrap();
        assert!(old.exists());
        assert!(!newer.exists());
        assert_eq!(
            fs::metadata(old.join("working.tiff"))
                .unwrap()
                .modified()
                .unwrap(),
            media
        );
    }

    #[test]
    fn limit_changes_and_thumbnail_entries_are_managed() {
        let temp = tempfile::tempdir().unwrap();
        let cache = cache(temp.path(), 50);
        let lease = cache.lease("aa", "previews").unwrap();
        fs::write(lease.path().join("preview.jpg"), [0; 20]).unwrap();
        assert_eq!(cache.status().unwrap().in_use_bytes, 20);
        drop(lease);
        cache.set_limit(10);
        assert_eq!(cache.status().unwrap().limit_bytes, 10);
        assert_eq!(cache.trim().unwrap().removed_bytes, 20);
    }

    #[test]
    fn restart_accounts_for_old_revisions_and_interrupted_downloads() {
        let temp = tempfile::tempdir().unwrap();
        let old = temp.path().join("aa/source-0000000000000001");
        let interrupted = temp.path().join("aa/source-0000000000000002");
        fs::create_dir_all(&old).unwrap();
        fs::create_dir_all(&interrupted).unwrap();
        fs::write(old.join("working.tiff"), [0; 12]).unwrap();
        fs::write(old.join("source.json"), b"{}").unwrap();
        fs::write(interrupted.join("unfinished-download"), [0; 8]).unwrap();
        let cache = cache(temp.path(), 10);
        assert_eq!(cache.status().unwrap().size_bytes, 22);
        let result = cache.trim().unwrap();
        assert!(result.remaining_bytes <= 10);
        assert_eq!(result.removed_bytes + result.remaining_bytes, 22);
        cache.clear().unwrap();
        assert!(!old.exists() && !interrupted.exists());
    }

    #[test]
    fn concurrent_clear_cannot_remove_a_decoders_source() {
        let temp = tempfile::tempdir().unwrap();
        let cache = cache(temp.path(), 0);
        let lease = cache.lease("aa", "source-0000000000000001").unwrap();
        fs::write(lease.path().join("working.tiff"), b"image bytes").unwrap();
        let (ready, started) = std::sync::mpsc::channel();
        let (finished, cleared) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            ready.send(()).unwrap();
            cleared.recv().unwrap();
            assert_eq!(
                fs::read(lease.path().join("working.tiff")).unwrap(),
                b"image bytes"
            );
            drop(lease);
        });
        started.recv().unwrap();
        assert_eq!(cache.clear().unwrap().protected_bytes, 11);
        finished.send(()).unwrap();
        reader.join().unwrap();
        assert_eq!(cache.clear().unwrap().removed_bytes, 11);
    }

    #[test]
    fn releasing_the_last_lease_automatically_enforces_the_limit() {
        let temp = tempfile::tempdir().unwrap();
        let (errors, reported) = std::sync::mpsc::channel();
        let cache = Arc::new(
            PhotoCache::new(temp.path().to_path_buf(), 10, move |error| {
                errors.send(error.to_string()).unwrap();
            })
            .unwrap(),
        );
        let lease = cache.lease("aa", "previews").unwrap();
        fs::write(lease.path().join("preview.jpg"), [0; 20]).unwrap();
        assert_eq!(cache.status().unwrap().in_use_bytes, 20);
        drop(lease);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while cache.status().unwrap().size_bytes > 10 {
            assert!(
                std::time::Instant::now() < deadline,
                "automatic pruning did not run"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(reported.try_recv().is_err());
        assert_eq!(cache.status().unwrap().size_bytes, 0);
    }

    #[test]
    fn failed_cleanup_is_reported_instead_of_claiming_success() {
        let temp = tempfile::tempdir().unwrap();
        let cache = cache(temp.path(), 0);
        let entry = entry(&cache, "aa", 10, 1);
        fs::create_dir(entry.join("unexpected-directory")).unwrap();
        assert!(cache.clear().unwrap_err().contains("Unexpected directory"));
        assert!(entry.join("working.tiff").exists());
    }

    #[test]
    fn rejects_unsafe_paths_and_ignores_unmanaged_files() {
        let temp = tempfile::tempdir().unwrap();
        let cache = cache(temp.path(), 0);
        assert!(cache.lease("../outside", "previews").is_err());
        assert!(cache.lease("aa", "../outside").is_err());
        fs::write(temp.path().join("keep.txt"), b"not managed").unwrap();
        cache.clear().unwrap();
        assert!(temp.path().join("keep.txt").exists());
    }

    #[cfg(unix)]
    #[test]
    fn never_follows_links_into_originals_or_edits() {
        let temp = tempfile::tempdir().unwrap();
        let outside = temp.path().join("originals");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("original.jpg"), b"original").unwrap();
        let cache = cache(&temp.path().join("cache"), 0);
        std::os::unix::fs::symlink(&outside, cache.root.join("aa")).unwrap();
        assert!(cache.lease("aa", "previews").is_err());
        assert!(cache.clear().is_err());
        assert_eq!(fs::read(outside.join("original.jpg")).unwrap(), b"original");
    }
}
