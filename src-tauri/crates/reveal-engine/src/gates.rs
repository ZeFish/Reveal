use std::sync::{Arc, Mutex};
use anyhow::Result;

/// Keeps threads that want the SAME uncached item from each computing it.
///
/// The cache lock is deliberately released before the work: holding it
/// would serialise every render behind every other one. What that release used to cost
/// was a duplicate compute whenever two threads raced.
///
/// The gate is per key, never global — different keys still compute in parallel —
/// and every caller re-checks the cache through it, because the thread ahead has
/// just filled it.
#[derive(Default)]
pub(crate) struct Gates<K: PartialEq + Clone>(pub(crate) Mutex<Vec<(K, Arc<Mutex<()>>)>>);

impl<K: PartialEq + Clone> Gates<K> {
    /// Run `compute` only if `lookup` still comes up empty once this thread
    /// holds the gate. Returns the result and whether THIS call produced it.
    pub(crate) fn once<L, D, R>(&self, key: &K, lookup: L, compute: D) -> Result<(R, bool)>
    where
        L: Fn() -> Option<R>,
        D: FnOnce() -> Result<R>,
    {
        if let Some(res) = lookup() {
            return Ok((res, false));
        }
        let t_gate = std::time::Instant::now();
        let gate = self.gate(key);
        let _held = gate.lock().unwrap_or_else(|e| e.into_inner());
        let gate_wait = t_gate.elapsed().as_millis();
        if gate_wait > 5 {
            eprintln!("[perf] gate: waited {} ms to acquire lock", gate_wait);
        }
        // Whoever held the gate before us has published their result.
        if let Some(res) = lookup() {
            return Ok((res, false));
        }
        Ok((compute()?, true))
    }

    /// The gate for one key, created on first ask and shared thereafter.
    fn gate(&self, key: &K) -> Arc<Mutex<()>> {
        let mut gates = self.0.lock().unwrap_or_else(|e| e.into_inner());
        // Drop gates nobody is queued on, so this list tracks tasks in
        // flight rather than every key ever requested.
        gates.retain(|(_, g)| Arc::strong_count(g) > 1);
        if let Some((_, g)) = gates.iter().find(|(k, _)| k == key) {
            return g.clone();
        }
        let g = Arc::new(Mutex::new(()));
        gates.push((key.clone(), g.clone()));
        g
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use spektrafilm_math::image::ImageBuf;

    type DecodeGates = Gates<(PathBuf, bool)>;

    fn frame() -> Arc<ImageBuf> {
        Arc::new(ImageBuf::from_data(1, 1, vec![0.5, 0.5, 0.5]))
    }

    /// The whole point: six threads wanting the same uncached photo must cost
    /// ONE decode. Before the gate they cost six, which is what pinned six
    /// cores inside LibRaw when flipping grid → dev → grid → dev.
    #[test]
    fn concurrent_asks_for_one_photo_decode_it_once() {
        let gates = DecodeGates::default();
        let cache: Mutex<Option<Arc<ImageBuf>>> = Mutex::new(None);
        let decodes = AtomicUsize::new(0);
        let key = (PathBuf::from("/nas/2026/A.RAF"), true);

        std::thread::scope(|scope| {
            for _ in 0..6 {
                scope.spawn(|| {
                    let (_img, _) = gates
                        .once(
                            &key,
                            || cache.lock().unwrap().clone(),
                            || {
                                decodes.fetch_add(1, Ordering::SeqCst);
                                // Stand in for LibRaw: long enough that every
                                // other thread is certainly already waiting.
                                std::thread::sleep(Duration::from_millis(50));
                                let img = frame();
                                *cache.lock().unwrap() = Some(img.clone());
                                Ok(img)
                            },
                        )
                        .unwrap();
                });
            }
        });

        assert_eq!(decodes.load(Ordering::SeqCst), 1);
    }

    /// And it must not have bought that by serialising the library: two
    /// different photos have to decode at the same time, or a prefetch would
    /// again block the photo on screen.
    #[test]
    fn two_photos_do_not_wait_for_each_other() {
        let gates = DecodeGates::default();
        let inside = AtomicUsize::new(0);
        let overlapped = AtomicUsize::new(0);
        // Shared by reference so `move` on the closure only moves the borrows.
        let (gates, inside, overlapped) = (&gates, &inside, &overlapped);

        std::thread::scope(|scope| {
            for name in ["/nas/A.RAF", "/nas/B.RAF"] {
                scope.spawn(move || {
                    let key = (PathBuf::from(name), true);
                    gates
                        .once(
                            &key,
                            || None,
                            || {
                                inside.fetch_add(1, Ordering::SeqCst);
                                std::thread::sleep(Duration::from_millis(80));
                                // Both threads sleep 80ms; if the gate were
                                // global, the second would enter only after
                                // the first had left and seen 1 here.
                                overlapped
                                    .fetch_max(inside.load(Ordering::SeqCst), Ordering::SeqCst);
                                inside.fetch_sub(1, Ordering::SeqCst);
                                Ok(frame())
                            },
                        )
                        .unwrap();
                });
            }
        });

        assert_eq!(overlapped.load(Ordering::SeqCst), 2);
    }

    /// The gate list tracks decodes in flight, not every photo ever opened —
    /// otherwise a long cull would grow it without bound.
    #[test]
    fn finished_gates_are_swept() {
        let gates = DecodeGates::default();
        for i in 0..50 {
            let key = (PathBuf::from(format!("/nas/{i}.RAF")), true);
            gates.once(&key, || None, || Ok(frame())).unwrap();
        }
        assert!(gates.0.lock().unwrap().len() <= 2);
    }
}
