//! How many files the app may hold open at once.
//!
//! An app launched from the Finder, the Dock or `open` inherits launchd's limit — **256** open
//! files on macOS (`launchctl limit maxfiles`) — where a Terminal starts at a million, which is
//! why it never showed in a shell. An import is the first thing to reach 256: each photo holds
//! its card file, its destination, the camera preview it is cached from and the directory scan,
//! while the grid decodes six thumbnails and a story or an AI pass reads more. Past the limit
//! every open fails with "Too many open files (os error 24)" — a photo that will not open, a
//! preview that will not save.
//!
//! Raised once at startup, as far as the system allows.

/// The ceiling we ask for. Plenty for the work above, and the most macOS accepts per process
/// (`OPEN_MAX`) without a kernel setting.
const WANTED: u64 = 10_240;

/// Raise the soft limit on open files to [`WANTED`], or to the hard limit if that is lower.
/// Returns the soft limit now in force, or `None` if the system refused to tell or to change it.
#[cfg(unix)]
pub fn raise_open_file_limit() -> Option<u64> {
    let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: `limit` is a valid, writable rlimit for the duration of both calls.
    unsafe {
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) != 0 {
            return None;
        }
        let hard = limit.rlim_max;
        let target = if hard == libc::RLIM_INFINITY { WANTED } else { WANTED.min(hard) };
        if limit.rlim_cur >= target {
            return Some(limit.rlim_cur);
        }
        limit.rlim_cur = target;
        if libc::setrlimit(libc::RLIMIT_NOFILE, &limit) != 0 {
            // macOS can refuse a high value even when the hard limit is "unlimited": settle for 4096.
            limit.rlim_cur = target.min(4096);
            if libc::setrlimit(libc::RLIMIT_NOFILE, &limit) != 0 {
                return None;
            }
        }
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) != 0 {
            return None;
        }
        Some(limit.rlim_cur)
    }
}

#[cfg(not(unix))]
pub fn raise_open_file_limit() -> Option<u64> {
    None
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn soft_limit() -> u64 {
        let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) };
        limit.rlim_cur
    }

    fn set_soft_limit(value: u64) {
        let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        unsafe {
            libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit);
            limit.rlim_cur = value;
            libc::setrlimit(libc::RLIMIT_NOFILE, &limit);
        }
    }

    #[test]
    fn a_finder_launched_256_is_raised() {
        // What launchd hands a GUI app. (Lowering the soft limit is always allowed.)
        set_soft_limit(256);
        assert_eq!(soft_limit(), 256);
        let now = raise_open_file_limit().expect("the limit can be read and raised");
        assert!(now >= 4096, "still only {now} open files");
        assert_eq!(now, soft_limit());
    }

    #[test]
    fn a_limit_already_high_enough_is_left_alone() {
        let before = soft_limit().max(WANTED);
        set_soft_limit(before);
        assert_eq!(raise_open_file_limit(), Some(before));
    }
}
