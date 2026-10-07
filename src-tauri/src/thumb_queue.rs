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

use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// How long a request may wait for a slot before it is given up on.
pub(crate) const MAX_WAIT: Duration = Duration::from_secs(20);

struct State {
    running: usize,
    /// Tickets of the requests waiting, in arrival order (the newest is last).
    waiting: Vec<u64>,
    next_ticket: u64,
}

pub(crate) struct ThumbQueue {
    state: Mutex<State>,
    cv: Condvar,
    slots: usize,
}

/// Held while a thumbnail decodes; frees the slot (and wakes the next request) on drop.
pub(crate) struct ThumbSlot<'a>(&'a ThumbQueue);

impl ThumbQueue {
    pub(crate) fn new(slots: usize) -> Self {
        Self {
            state: Mutex::new(State { running: 0, waiting: Vec::new(), next_ticket: 0 }),
            cv: Condvar::new(),
            slots,
        }
    }

    /// Wait for a slot. `None` means the wait ran out: nobody should be waiting for this
    /// picture any more, so do not decode it.
    pub(crate) fn acquire(&self, max_wait: Duration) -> Option<ThumbSlot<'_>> {
        let deadline = Instant::now() + max_wait;
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let ticket = state.next_ticket;
        state.next_ticket += 1;
        state.waiting.push(ticket);
        loop {
            let newest = state.waiting.last().copied() == Some(ticket);
            if state.running < self.slots && newest {
                state.waiting.pop();
                state.running += 1;
                return Some(ThumbSlot(self));
            }
            let now = Instant::now();
            if now >= deadline {
                state.waiting.retain(|t| *t != ticket);
                // Leaving can make the next-newest the one that should go.
                self.cv.notify_all();
                return None;
            }
            let (guard, _) = self
                .cv
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(|e| e.into_inner());
            state = guard;
        }
    }

    fn release(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.running -= 1;
        self.cv.notify_all();
    }
}

impl Drop for ThumbSlot<'_> {
    fn drop(&mut self) {
        self.0.release();
    }
}

pub(crate) struct ThumbQueueState(pub(crate) std::sync::Arc<ThumbQueue>);

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn the_newest_waiting_request_goes_first() {
        let queue = Arc::new(ThumbQueue::new(1));
        let held = queue.acquire(Duration::from_secs(1)).expect("a free slot");
        let order = Arc::new(Mutex::new(Vec::new()));
        let mut threads = Vec::new();
        for name in ["old", "newer", "newest"] {
            let (queue, order) = (queue.clone(), order.clone());
            threads.push(std::thread::spawn(move || {
                let slot = queue.acquire(Duration::from_secs(5)).expect("served in the end");
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
        let _held = queue.acquire(Duration::from_secs(1)).expect("a free slot");
        assert!(queue.acquire(Duration::from_millis(80)).is_none());
    }

    #[test]
    fn a_dropped_request_does_not_block_the_one_behind_it() {
        let queue = Arc::new(ThumbQueue::new(1));
        let held = queue.acquire(Duration::from_secs(1)).expect("a free slot");
        let q = queue.clone();
        let behind = std::thread::spawn(move || q.acquire(Duration::from_secs(5)).is_some());
        std::thread::sleep(Duration::from_millis(50));
        // A newer request arrives, gives up, and leaves.
        assert!(queue.acquire(Duration::from_millis(50)).is_none());
        drop(held);
        assert!(behind.join().unwrap());
    }
}
