//! Who gets to decode a grid thumbnail next.
//!
//! Scrolling a big folder mounts a few dozen cells per second and unmounts as many. Each
//! mounted cell asks for its picture; a cell that has scrolled away no longer wants it, but
//! its request is still waiting here. Served first-come-first-served, the cells on screen
//! sat behind everything that had scrolled past — the screen you stopped on was the last to
//! fill in.
//!
//! The rule, in one place:
//!   * **Newest first.** When a slot frees up, the most recent request takes it — and the most
//!     recent requests are the cells on screen.
//!   * **What nobody is waiting for any more is dropped.** A request that has stood in line for
//!     [`MAX_WAIT`] without a slot is released with `None`; the cell is long gone, or if it is
//!     still there it asks again (its retry, or a click on it). Without this a long scroll left
//!     thousands of blocked threads holding the blocking pool.
//!   * **The rest continues afterwards.** Older requests still run when no newer one is
//!     waiting, and their results land in the local cache, so scrolling back finds them there.

use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// How long a request may wait for a slot before it is given up on.
pub(crate) const MAX_WAIT: Duration = Duration::from_secs(20);

struct State {
    running: usize,
    /// Of those running, how many are cells on screen.
    running_visible: usize,
    /// The requests waiting, in arrival order (the newest is last): ticket and photo.
    waiting: Vec<(u64, String)>,
    next_ticket: u64,
    /// The photos whose cell is on screen right now, as the grid last said.
    visible: HashSet<String>,
}

impl State {
    /// The ticket that goes next: a cell on screen before any other, then the newest.
    fn best(&self) -> Option<u64> {
        self.waiting
            .iter()
            .max_by_key(|(ticket, path)| (self.visible.contains(path), *ticket))
            .map(|(ticket, _)| *ticket)
    }

    fn visible_waiting(&self) -> bool {
        self.waiting.iter().any(|(_, path)| self.visible.contains(path))
    }
}

pub(crate) struct ThumbQueue {
    state: Mutex<State>,
    cv: Condvar,
    /// How many thumbnails decode at once right now: `normal`, or [`BACKGROUND_SLOTS`] while a
    /// photo is being developed.
    slots: AtomicUsize,
    normal: usize,
    /// How many pieces of foreground work (a RAW being read for Develop) are in flight. While
    /// there is any, no new thumbnail starts at all: on a NAS the wire is the whole budget.
    foreground: AtomicUsize,
    /// How many grid selections are being warmed (a RAW read so that Enter opens it ready). The
    /// thumbnails of cells on screen go before it, everything else after.
    warming: AtomicUsize,
    /// How many of those are the photo the person is working on (not a neighbour being warmed).
    /// A neighbour waits for these to finish before it reads anything.
    current: AtomicUsize,
    /// The photo the person last worked on; a neighbour warmed for another one is no use any more.
    current_path: Mutex<String>,
    /// One neighbour reads at a time.
    neighbour_turn: Mutex<()>,
}

/// While a photo is open in Develop its RAW is being read and decoded, often from the same NAS
/// link the grid's thumbnails use (each a multi-megabyte embedded JPEG). The grid keeps filling
/// in, but one at a time, so the photo the person is working on is not left to share the wire.
pub(crate) const BACKGROUND_SLOTS: usize = 1;

/// Held while foreground work runs; lets the thumbnails go on when it ends.
pub(crate) struct ForegroundGuard<'a> {
    queue: &'a ThumbQueue,
    current: bool,
    _turn: Option<std::sync::MutexGuard<'a, ()>>,
}

impl Drop for ForegroundGuard<'_> {
    fn drop(&mut self) {
        self.queue.foreground.fetch_sub(1, Ordering::SeqCst);
        if self.current {
            self.queue.current.fetch_sub(1, Ordering::SeqCst);
        }
        let _guard = self.queue.state.lock().unwrap_or_else(|e| e.into_inner());
        self.queue.cv.notify_all();
    }
}

/// Held while a thumbnail decodes; frees the slot (and wakes the next request) on drop.
pub(crate) struct ThumbSlot<'a> {
    queue: &'a ThumbQueue,
    visible: bool,
}

impl ThumbQueue {
    pub(crate) fn new(slots: usize) -> Self {
        Self {
            state: Mutex::new(State { running: 0, running_visible: 0, waiting: Vec::new(), next_ticket: 0, visible: HashSet::new() }),
            cv: Condvar::new(),
            slots: AtomicUsize::new(slots),
            normal: slots,
            foreground: AtomicUsize::new(0),
            warming: AtomicUsize::new(0),
            current: AtomicUsize::new(0),
            current_path: Mutex::new(String::new()),
            neighbour_turn: Mutex::new(()),
        }
    }

    /// Give the foreground work the wire (`true`), or hand it back to the grid (`false`).
    pub(crate) fn set_background(&self, background: bool) {
        let slots = if background { self.normal.min(BACKGROUND_SLOTS) } else { self.normal };
        self.slots.store(slots, Ordering::SeqCst);
        eprintln!("[perf] thumb queue: {} slot(s) ({})", slots, if background { "develop open" } else { "grid" });
        // Raising the limit lets waiting requests start; lowering it only affects new starts.
        let _guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        self.cv.notify_all();
    }

    /// The thumbnails that may run at once right now.
    fn limit(&self) -> usize {
        if self.foreground.load(Ordering::SeqCst) > 0 { 0 } else { self.slots.load(Ordering::SeqCst) }
    }

    /// Foreground work starts: no new thumbnail begins until the guard drops. Ones already
    /// reading finish (they cannot be cancelled), so this is how Develop gets the wire.
    pub(crate) fn foreground(&self, path: &str) -> ForegroundGuard<'_> {
        {
            let mut current = self.current_path.lock().unwrap_or_else(|e| e.into_inner());
            if *current != path {
                *current = path.to_string();
            }
        }
        self.foreground.fetch_add(1, Ordering::SeqCst);
        self.current.fetch_add(1, Ordering::SeqCst);
        ForegroundGuard { queue: self, current: true, _turn: None }
    }

    /// Warming a neighbour: waits until the photo being worked on is done (at most `max_wait`),
    /// then holds the wire like any foreground work, one neighbour at a time. `None` if the
    /// person moved to another photo in the meantime: warming this one would only slow the new
    /// photo down.
    pub(crate) fn foreground_after_current(&self, max_wait: Duration) -> Option<ForegroundGuard<'_>> {
        let anchor = self.current_path.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let deadline = Instant::now() + max_wait;
        let turn = self.neighbour_turn.lock().unwrap_or_else(|e| e.into_inner());
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        while self.current.load(Ordering::SeqCst) > 0 {
            let now = Instant::now();
            if now >= deadline {
                return None;
            }
            let (guard, _) = self.cv.wait_timeout(state, deadline - now).unwrap_or_else(|e| e.into_inner());
            state = guard;
        }
        drop(state);
        if *self.current_path.lock().unwrap_or_else(|e| e.into_inner()) != anchor {
            return None;
        }
        self.foreground.fetch_add(1, Ordering::SeqCst);
        Some(ForegroundGuard { queue: self, current: false, _turn: Some(turn) })
    }

    /// The grid says which photos are on screen. Their thumbnails go first; waiting requests
    /// re-check at once.
    pub(crate) fn set_visible(&self, paths: Vec<String>) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.visible = paths.into_iter().collect();
        self.cv.notify_all();
    }

    /// Wait for a slot. `None` means the wait ran out: nobody should be waiting for this
    /// picture any more, so do not decode it.
    ///
    /// The order: cells on screen first; then, for the rest, only when no cell on screen is
    /// waiting or reading, nothing is being warmed for the selection and nothing is in the
    /// foreground. Within a class the newest request goes first.
    pub(crate) fn acquire(&self, path: &str, max_wait: Duration) -> Option<ThumbSlot<'_>> {
        let deadline = Instant::now() + max_wait;
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let ticket = state.next_ticket;
        state.next_ticket += 1;
        state.waiting.push((ticket, path.to_string()));
        loop {
            let visible = state.visible.contains(path);
            let allowed = if visible {
                state.running < self.limit()
            } else {
                state.running < self.limit()
                    && state.running_visible == 0
                    && !state.visible_waiting()
                    && self.warming.load(Ordering::SeqCst) == 0
            };
            if allowed && state.best() == Some(ticket) {
                state.waiting.retain(|(t, _)| *t != ticket);
                state.running += 1;
                if visible {
                    state.running_visible += 1;
                }
                return Some(ThumbSlot { queue: self, visible });
            }
            let now = Instant::now();
            if now >= deadline {
                state.waiting.retain(|(t, _)| *t != ticket);
                // Leaving can make the next one the one that should go.
                self.cv.notify_all();
                return None;
            }
            let (guard, _) = self.cv.wait_timeout(state, deadline - now).unwrap_or_else(|e| e.into_inner());
            state = guard;
        }
    }

    /// Warming the selected photo's RAW: after the cells on screen have their pictures
    /// (at most `max_wait`), and while it reads, the thumbnails of cells off screen wait.
    /// Cells that come on screen meanwhile still go first.
    pub(crate) fn warming_after_visible(&self, max_wait: Duration) -> WarmingGuard<'_> {
        let deadline = Instant::now() + max_wait;
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        while state.visible_waiting() || state.running_visible > 0 {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            let (guard, _) = self.cv.wait_timeout(state, deadline - now).unwrap_or_else(|e| e.into_inner());
            state = guard;
        }
        self.warming.fetch_add(1, Ordering::SeqCst);
        WarmingGuard { queue: self }
    }

    /// `running/slots, waiting` for the log.
    pub(crate) fn describe(&self) -> String {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        format!("{}/{} running, {} waiting", state.running, self.limit(), state.waiting.len())
    }

    fn release(&self, visible: bool) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.running -= 1;
        if visible {
            state.running_visible -= 1;
        }
        self.cv.notify_all();
    }
}

impl Drop for ThumbSlot<'_> {
    fn drop(&mut self) {
        self.queue.release(self.visible);
    }
}

/// Held while the selected photo's RAW is being warmed; lets the off-screen thumbnails go on after.
pub(crate) struct WarmingGuard<'a> {
    queue: &'a ThumbQueue,
}

impl Drop for WarmingGuard<'_> {
    fn drop(&mut self) {
        self.queue.warming.fetch_sub(1, Ordering::SeqCst);
        let _guard = self.queue.state.lock().unwrap_or_else(|e| e.into_inner());
        self.queue.cv.notify_all();
    }
}

pub(crate) struct ThumbQueueState(pub(crate) std::sync::Arc<ThumbQueue>);

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn a_photo_in_develop_leaves_the_grid_one_slot() {
        let queue = ThumbQueue::new(6);
        queue.set_background(true);
        let first = queue.acquire("", Duration::from_millis(50)).expect("one slot is left");
        assert!(queue.acquire("", Duration::from_millis(50)).is_none(), "a second one waits");
        drop(first);
        queue.set_background(false);
        let held: Vec<_> = (0..6).map(|_| queue.acquire("", Duration::from_millis(50)).expect("six again")).collect();
        assert_eq!(held.len(), 6);
    }

    #[test]
    fn foreground_work_stops_new_thumbnails_until_it_ends() {
        let queue = ThumbQueue::new(6);
        let work = queue.foreground("/a.raw");
        assert!(queue.acquire("", Duration::from_millis(50)).is_none(), "nothing starts meanwhile");
        drop(work);
        assert!(queue.acquire("", Duration::from_millis(50)).is_some(), "and everything resumes");
    }

    #[test]
    fn a_neighbour_waits_for_the_photo_in_hand() {
        let queue = Arc::new(ThumbQueue::new(6));
        let work = queue.foreground("/a.raw");
        let waiter = {
            let queue = queue.clone();
            std::thread::spawn(move || {
                let t = Instant::now();
                let _g = queue.foreground_after_current(Duration::from_secs(5));
                assert!(_g.is_some());
                t.elapsed()
            })
        };
        std::thread::sleep(Duration::from_millis(150));
        drop(work);
        let waited = waiter.join().unwrap();
        assert!(waited >= Duration::from_millis(140), "it held back: {waited:?}");
    }

    #[test]
    fn cells_on_screen_go_before_the_others_and_the_warm_waits_for_them() {
        let queue = Arc::new(ThumbQueue::new(1));
        queue.set_visible(vec!["/seen.raw".to_string()]);
        let held = queue.acquire("/seen.raw", Duration::from_secs(1)).expect("a free slot");
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut threads = Vec::new();
        // An off-screen cell asks first, then another cell on screen: the one on screen wins.
        for (name, path) in [("away", "/away.raw"), ("seen2", "/seen.raw")] {
            let (queue, order) = (queue.clone(), order.clone());
            threads.push(std::thread::spawn(move || {
                let slot = queue.acquire(path, Duration::from_secs(5)).expect("served in the end");
                order.lock().unwrap().push(name);
                drop(slot);
            }));
            std::thread::sleep(Duration::from_millis(60));
        }
        // The warm waits while a cell on screen is reading or waiting.
        let warm = {
            let queue = queue.clone();
            let order = order.clone();
            std::thread::spawn(move || {
                let _g = queue.warming_after_visible(Duration::from_secs(5));
                order.lock().unwrap().push("warm");
            })
        };
        std::thread::sleep(Duration::from_millis(60));
        drop(held);
        for t in threads {
            t.join().unwrap();
        }
        warm.join().unwrap();
        let order = order.lock().unwrap().clone();
        let pos = |n: &str| order.iter().position(|x| *x == n).unwrap();
        assert!(pos("seen2") < pos("away"), "a cell on screen first: {order:?}");
        assert!(pos("seen2") < pos("warm"), "and before the warm: {order:?}");
    }

    #[test]
    fn the_newest_waiting_request_goes_first() {
        let queue = Arc::new(ThumbQueue::new(1));
        let held = queue.acquire("", Duration::from_secs(1)).expect("a free slot");
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut threads = Vec::new();
        for name in ["old", "newer", "newest"] {
            let (queue, order) = (queue.clone(), order.clone());
            threads.push(std::thread::spawn(move || {
                let slot = queue.acquire("", Duration::from_secs(5)).expect("served in the end");
                order.lock().unwrap().push(name);
                drop(slot);
            }));
            // Arrival order matters here: let each one reach the queue before the next starts.
            std::thread::sleep(Duration::from_millis(60));
        }
        drop(held);
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(*order.lock().unwrap(), vec!["newest", "newer", "old"]);
    }

    #[test]
    fn a_request_nobody_waits_for_any_more_is_dropped() {
        let queue = ThumbQueue::new(1);
        let _held = queue.acquire("", Duration::from_secs(1)).expect("a free slot");
        assert!(queue.acquire("", Duration::from_millis(80)).is_none());
    }

    #[test]
    fn a_dropped_request_does_not_block_the_one_behind_it() {
        let queue = Arc::new(ThumbQueue::new(1));
        let held = queue.acquire("", Duration::from_secs(1)).expect("a free slot");
        let q = queue.clone();
        let behind = std::thread::spawn(move || q.acquire("", Duration::from_secs(5)).is_some());
        std::thread::sleep(Duration::from_millis(50));
        // A newer request arrives, gives up, and leaves.
        assert!(queue.acquire("", Duration::from_millis(50)).is_none());
        drop(held);
        assert!(behind.join().unwrap());
    }
}
