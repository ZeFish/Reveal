//! The one rule for writing a person's data to a disk that may be a NAS.
//!
//! Reveal's files live on an NFS share that times out now and then (`os error 60`),
//! goes stale (`ESTALE`), or accepts a write into the page cache that never
//! reaches the server (a 790-byte story note once came back as pure 0x00). Every
//! place that wrote its own way (a bare `fs::write`, a temp file with no
//! read-back, a retry in one layer and none in the next) failed in its own way.
//! They now share two things:
//!
//! * [`retry`] — run an operation again when the error is the kind a NAS
//!   recovers from, at fixed, short delays, and not at all for the errors it does not
//!   recover from (no such file, permission, disk full).
//! * [`write_durable`] — write a file so that what is on disk is either the old
//!   version or the complete new one, never a half: temp file, `fsync`, read it
//!   back and compare, then rename over the target. The whole sequence is retried.
//!
//! The recipe save in the webview (`saveRecipeSoon`) keeps its own, slower retry on
//! top: it queues behind the person's edits and reports to them, which a write
//! deep in Rust cannot.

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Waits between attempts; the first attempt has none. About six seconds in all —
/// long enough for a NAS blip, short enough that a person waiting on a save is not
/// left hanging by the transient cases.
pub const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_millis(400),
    Duration::from_millis(1500),
    Duration::from_millis(4000),
];

/// Is this the kind of failure a second try can fix? macOS errno values are
/// listed by number because `std::io::ErrorKind` has no variant for most of them.
pub fn is_transient(error: &io::Error) -> bool {
    use io::ErrorKind::*;
    if matches!(
        error.kind(),
        TimedOut | Interrupted | WouldBlock | ConnectionReset | ConnectionAborted | NotConnected | BrokenPipe
    ) {
        return true;
    }
    matches!(
        error.raw_os_error(),
        // EIO, EAGAIN, ENETDOWN, ENETUNREACH, ECONNABORTED, ECONNRESET,
        // ETIMEDOUT, EHOSTDOWN, EHOSTUNREACH, ESTALE
        Some(5 | 35 | 50 | 51 | 53 | 54 | 60 | 64 | 65 | 70)
    )
}

/// Run `op`; on a transient error, wait and run it again, once per entry of
/// `delays`. Any other error is returned at once. The last error is returned if
/// every attempt fails. `sleep` is a parameter so the policy can be tested without
/// waiting.
pub fn retry_with<T>(
    delays: &[Duration],
    sleep: impl Fn(Duration),
    mut op: impl FnMut() -> io::Result<T>,
) -> io::Result<T> {
    let mut result = op();
    for delay in delays {
        match &result {
            Err(e) if is_transient(e) => {
                sleep(*delay);
                result = op();
            }
            _ => break,
        }
    }
    result
}

/// [`retry_with`] at [`RETRY_DELAYS`], really sleeping.
pub fn retry<T>(op: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    retry_with(&RETRY_DELAYS, std::thread::sleep, op)
}

/// Write `bytes` to `path` so a reader sees the old file or the whole new one.
///
/// Makes the parent folder, writes a sibling temp file, `fsync`s it, reads it back
/// and compares (a write the server silently dropped shows up here, not as a
/// corrupted note later), and only then renames it over `path`. A mismatch leaves
/// `path` untouched. The whole sequence is retried on transient errors, and a
/// leftover temp file is removed on failure.
pub fn write_durable(path: &Path, bytes: &[u8]) -> io::Result<()> {
    retry(|| write_durable_once(path, bytes))
}

fn write_durable_once(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let tmp = temp_sibling(path);
    let result = (|| {
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(bytes)?;
            f.sync_all()?;
        }
        if std::fs::read(&tmp)? != bytes {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "write verification failed (read-back mismatch): the file on disk was left untouched",
            ));
        }
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Like [`write_durable`], but leaves the file alone when it already holds exactly
/// these bytes — so reopening a developed photo never bumps its modification time.
pub fn write_durable_if_changed(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Ok(existing) = std::fs::read(path) {
        if existing == bytes {
            return Ok(());
        }
    }
    write_durable(path, bytes)
}

fn temp_sibling(path: &Path) -> PathBuf {
    // Hidden, so a folder scan never mistakes it for a photo or a sidecar.
    let mut name = std::ffi::OsString::from(".");
    name.push(path.file_name().unwrap_or_default());
    // One name per write: two writers of the same file (the grid persisting a thumbnail while a
    // develop publishes) used to share a temp file, and whichever renamed second found it gone
    // ("No such file or directory").
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    name.push(format!(".{}-{n}.reveal-tmp", std::process::id()));
    path.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    fn timed_out() -> io::Error {
        io::Error::from(io::ErrorKind::TimedOut)
    }

    #[test]
    fn a_transient_failure_is_retried_until_it_lands() {
        let sleeps = RefCell::new(Vec::new());
        let calls = Cell::new(0);
        let r = retry_with(&RETRY_DELAYS, |d| sleeps.borrow_mut().push(d), || {
            calls.set(calls.get() + 1);
            if calls.get() < 3 { Err(timed_out()) } else { Ok(7) }
        });
        assert_eq!(r.unwrap(), 7);
        assert_eq!(calls.get(), 3);
        assert_eq!(sleeps.borrow().len(), 2, "waits between tries, not after the one that worked");
    }

    #[test]
    fn a_transient_failure_that_never_lands_returns_the_last_error() {
        let calls = Cell::new(0);
        let r: io::Result<()> = retry_with(&RETRY_DELAYS, |_| {}, || {
            calls.set(calls.get() + 1);
            Err(timed_out())
        });
        assert_eq!(r.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert_eq!(calls.get(), 1 + RETRY_DELAYS.len());
    }

    #[test]
    fn an_error_a_second_try_cannot_fix_is_not_retried() {
        for kind in [io::ErrorKind::NotFound, io::ErrorKind::PermissionDenied, io::ErrorKind::InvalidInput] {
            let calls = Cell::new(0);
            let slept = Cell::new(false);
            let r: io::Result<()> = retry_with(&RETRY_DELAYS, |_| slept.set(true), || {
                calls.set(calls.get() + 1);
                Err(io::Error::from(kind))
            });
            assert!(r.is_err());
            assert_eq!(calls.get(), 1, "{kind:?} must not be retried");
            assert!(!slept.get());
        }
        let disk_full = io::Error::from_raw_os_error(28); // ENOSPC
        assert!(!is_transient(&disk_full), "a full disk does not clear in six seconds");
    }

    #[test]
    fn the_nfs_errno_values_count_as_transient() {
        for errno in [5, 35, 60, 70] {
            assert!(is_transient(&io::Error::from_raw_os_error(errno)), "errno {errno}");
        }
    }

    #[test]
    fn a_durable_write_lands_whole_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a/b/note.md");
        write_durable(&path, b"hello").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello");
        write_durable(&path, b"hello again").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello again");
        let names: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names.len(), 1, "only the file itself: {names:?}");
    }

    #[test]
    fn writing_the_same_bytes_again_does_not_touch_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.jpg");
        write_durable_if_changed(&path, b"jpeg").unwrap();
        let first = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::thread::sleep(Duration::from_millis(1100));
        write_durable_if_changed(&path, b"jpeg").unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), first);
        write_durable_if_changed(&path, b"jpeg2").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"jpeg2");
    }
}
