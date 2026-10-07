//! What has to be written back to a photo's folder, and when.
//!
//! The photo's folder is on a NAS that times out, so every write there is expensive, and the person
//! is often only *looking*: trying Rapid, then Spektra, then None, then Rapid again. Writing at
//! each step costs the NAS three round trips for nothing, and undoes them as often. The picture on
//! screen never waits for the disk; the disk waits for the person to stop.
//!
//! This module is that wait, in one place. A write is **queued** for a photo and **debounced**: each
//! new request restarts the quiet period ([`QUIET`]), a request for the opposite can **cancel** it
//! (choosing an engine again cancels the clearing of its settings, and the NAS is never touched),
//! and whatever is still waiting when the app closes is **flushed** rather than lost.
//!
//! It is the Rust counterpart of the `Photo` model of the interface (modules/core/models/Photo.js):
//! that one says what a photo *looks like*; this one says what is *owed to its files*.
//!
//! What a photo owes is a small record, not a single write: the develop settings (a recipe to save,
//! or a clearing — the latest of the two wins: Rapid, then None, then Spektra is one recipe saved)
//! and the rating (independent of them: setting stars never drops a recipe waiting to be written).
//! The quiet period that preview publishing waits out (see `preview.rs`) is the same constant.

use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use reveal_engine::Recipe;

/// How long a photo must be left alone before what is owed to its files is written.
pub(crate) const QUIET: Duration = Duration::from_millis(1500);

// ── the schedule: pure, so it can be tested with a clock we hold ─────────────────────────────

/// One change asked of a photo.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Change {
    /// Write this recipe (the engine and its settings) into the photo's sidecar.
    SaveRecipe(Box<Recipe>),
    /// Take the develop settings off the photo (the switch to "None").
    ClearDevelopment,
    /// Set the star rating (0–5).
    SetRating(u8),
}

/// What the develop settings are owed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Develop {
    Save(Box<Recipe>),
    Clear,
}

/// Everything one photo is owed: the develop settings and the rating, each at its latest.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Owed {
    pub(crate) develop: Option<Develop>,
    pub(crate) rating: Option<u8>,
}

impl Owed {
    /// Fold a new request in: the latest develop request replaces the earlier one, the latest
    /// rating replaces the earlier one, and neither touches the other.
    fn apply(&mut self, change: Change) {
        match change {
            Change::SaveRecipe(recipe) => self.develop = Some(Develop::Save(recipe)),
            Change::ClearDevelopment => self.develop = Some(Develop::Clear),
            Change::SetRating(stars) => self.rating = Some(stars),
        }
    }
}

/// Which photos owe a write, what, and from when. Knows nothing about threads or disks.
#[derive(Default)]
pub(crate) struct Schedule {
    due: HashMap<String, (Instant, Owed)>,
}

impl Schedule {
    /// A request for `path`: folded into what the photo already owes, and due `quiet` after `now`
    /// — a later request for the same photo pushes the due time back. That is the debounce.
    pub(crate) fn queue(&mut self, path: &str, change: Change, now: Instant, quiet: Duration) {
        let entry = self.due.entry(path.to_string()).or_insert_with(|| (now, Owed::default()));
        entry.0 = now + quiet;
        entry.1.apply(change);
    }

    /// Withdraw the develop settings' part of the debt (an engine was chosen again): `true` if
    /// there was one. A rating still owed stays owed.
    pub(crate) fn cancel_develop(&mut self, path: &str) -> bool {
        let Some((_, owed)) = self.due.get_mut(path) else { return false };
        let had = owed.develop.take().is_some();
        if *owed == Owed::default() {
            self.due.remove(path);
        }
        had
    }

    /// The writes whose quiet period is over, removed from the schedule.
    pub(crate) fn take_due(&mut self, now: Instant) -> Vec<(String, Owed)> {
        let ready: Vec<String> = self
            .due
            .iter()
            .filter(|(_, (at, _))| *at <= now)
            .map(|(path, _)| path.clone())
            .collect();
        ready
            .into_iter()
            .filter_map(|path| self.due.remove(&path).map(|(_, owed)| (path, owed)))
            .collect()
    }

    /// What one photo owes, removed (something is about to read it, and must not see it stale).
    pub(crate) fn take(&mut self, path: &str) -> Option<Owed> {
        self.due.remove(path).map(|(_, owed)| owed)
    }

    /// Everything still waiting, removed (the app is closing).
    pub(crate) fn take_all(&mut self) -> Vec<(String, Owed)> {
        self.due.drain().map(|(path, (_, owed))| (path, owed)).collect()
    }

    pub(crate) fn next_due(&self) -> Option<Instant> {
        self.due.values().map(|(at, _)| *at).min()
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.due.len()
    }
}

// ── the writer ───────────────────────────────────────────────────────────────────────────────

/// What a write does to the disk. A trait so the timing can be tested without one.
pub(crate) trait Disk: Send + Sync + 'static {
    /// Write the recipe into the photo's sidecar: the engine, and its settings.
    fn save_recipe(&self, path: &str, recipe: &Recipe) -> Result<(), String>;
    /// Take the develop settings off the photo: the recipe in its sidecar, and the developed
    /// preview, so the grid and the viewer fall back to the camera's own picture.
    fn clear_development(&self, path: &str) -> Result<(), String>;
    /// Write the star rating into the photo's sidecar.
    fn set_rating(&self, path: &str, stars: u8) -> Result<(), String>;
}

struct State {
    schedule: Schedule,
    closed: bool,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    quiet: Duration,
    disk: Box<dyn Disk>,
}

/// The queue of what is owed to photos' files, and the thread that pays it once things are calm.
#[derive(Clone)]
pub(crate) struct PhotoWrites {
    shared: Arc<Shared>,
}

impl PhotoWrites {
    pub(crate) fn start(disk: impl Disk, quiet: Duration) -> Self {
        let shared = Arc::new(Shared {
            state: Mutex::new(State { schedule: Schedule::default(), closed: false }),
            wake: Condvar::new(),
            quiet,
            disk: Box::new(disk),
        });
        let worker = shared.clone();
        std::thread::Builder::new()
            .name("photo-writes".into())
            .spawn(move || worker.run())
            .expect("the photo-writes thread starts");
        Self { shared }
    }

    /// Owe the photo a clearing of its develop settings, to be paid once it has been left alone.
    ///
    /// Also claims the photo's publish generation right now: a render that settled a moment ago
    /// may still be waiting to publish its `.preview.jpg`, which would bring back the file this
    /// clearing is going to remove. (Choosing an engine again claims a newer generation of its own.)
    pub(crate) fn queue_clear(&self, path: &str) {
        let _ = crate::preview::next_publish_generation(path);
        self.queue(path, Change::ClearDevelopment);
    }

    /// Owe the photo this recipe. Every edit calls this; only the last one is written, once the
    /// photo has been left alone — and if the settings are cleared meanwhile, that replaces it.
    pub(crate) fn queue_save_recipe(&self, path: &str, recipe: Recipe) {
        self.queue(path, Change::SaveRecipe(Box::new(recipe)));
    }

    /// Owe the photo this star rating (0–5). The latest one is written, once the photo is left alone.
    pub(crate) fn queue_set_rating(&self, path: &str, stars: u8) {
        self.queue(path, Change::SetRating(stars.min(5)));
    }

    fn queue(&self, path: &str, change: Change) {
        let mut state = self.shared.lock();
        state.schedule.queue(path, change, Instant::now(), self.shared.quiet);
        self.shared.wake.notify_all();
    }

    /// The person chose an engine again: withdraw the develop settings that were owed (a clearing,
    /// or a recipe about to be replaced). A rating still owed stays owed. `true` if something was.
    pub(crate) fn cancel(&self, path: &str) -> bool {
        self.shared.lock().schedule.cancel_develop(path)
    }

    /// Pay what this one photo owes, now — before something reads its files and must not see them
    /// stale (opening the photo reads its sidecar).
    pub(crate) fn flush(&self, path: &str) {
        let owed = self.shared.lock().schedule.take(path);
        if let Some(owed) = owed {
            self.shared.pay(path, &owed);
        }
    }

    /// Pay everything owed, now. For the app closing, where waiting would mean losing it.
    pub(crate) fn flush_all(&self) {
        let owed = self.shared.lock().schedule.take_all();
        for (path, owed) in owed {
            self.shared.pay(&path, &owed);
        }
    }

    #[cfg(test)]
    pub(crate) fn pending(&self) -> usize {
        self.shared.lock().schedule.len()
    }

    /// Stop the thread (after it has paid what is due). Tests only: the app's queue lives as long as it does.
    #[cfg(test)]
    pub(crate) fn close(&self) {
        self.shared.lock().closed = true;
        self.shared.wake.notify_all();
    }
}

impl Shared {
    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn pay(&self, path: &str, owed: &Owed) {
        let started = Instant::now();
        let develop = match &owed.develop {
            Some(Develop::Save(recipe)) => Some(self.disk.save_recipe(path, recipe)),
            Some(Develop::Clear) => Some(self.disk.clear_development(path)),
            None => None,
        };
        let rating = owed.rating.map(|stars| self.disk.set_rating(path, stars));
        eprintln!(
            "[perf] photo-writes {path}: paid in {} ms (develop: {}, rating: {})",
            started.elapsed().as_millis(),
            owed.develop.is_some(),
            owed.rating.is_some()
        );
        // One failing write does not stop the other, and neither stops the queue.
        for result in [develop, rating].into_iter().flatten() {
            if let Err(e) = result {
                eprintln!("photo-writes: could not write {path}: {e}");
            }
        }
    }

    fn run(&self) {
        let mut state = self.lock();
        loop {
            if state.closed {
                return;
            }
            let now = Instant::now();
            let due = state.schedule.take_due(now);
            if !due.is_empty() {
                // Pay without the lock: a slow NAS must not stop new requests from being queued.
                drop(state);
                for (path, owed) in &due {
                    self.pay(path, owed);
                }
                state = self.lock();
                continue;
            }
            state = match state.schedule.next_due() {
                Some(at) => {
                    self.wake
                        .wait_timeout(state, at.saturating_duration_since(now))
                        .unwrap_or_else(|e| e.into_inner())
                        .0
                }
                None => self.wake.wait(state).unwrap_or_else(|e| e.into_inner()),
            };
        }
    }
}

// ── the real disk ────────────────────────────────────────────────────────────────────────────

/// The disk of the running app: clears the files, then tells the interface so the grid redraws
/// the photo as shot.
pub(crate) struct AppDisk {
    pub(crate) app: tauri::AppHandle,
}

impl Disk for AppDisk {
    fn save_recipe(&self, path: &str, recipe: &Recipe) -> Result<(), String> {
        let result = crate::photo::Photo::new(path).save_recipe(recipe);
        if let Err(e) = &result {
            // reveal-io has already retried what a NAS recovers from: this one is lost, say so.
            crate::app_error(&self.app, format!("Could not save development settings: {e}"));
        }
        result
    }

    fn clear_development(&self, path: &str) -> Result<(), String> {
        use tauri::Emitter;
        crate::photo::Photo::new(path).clear_development_now()?;
        let _ = self.app.emit("preview-cleared", serde_json::json!({ "path": path }));
        Ok(())
    }

    fn set_rating(&self, path: &str, stars: u8) -> Result<(), String> {
        let result = crate::photo::Photo::new(path).set_rating(stars);
        if let Err(e) = &result {
            crate::app_error(&self.app, format!("Could not save the rating: {e}"));
        }
        result
    }
}

pub(crate) struct PhotoWritesState(pub(crate) PhotoWrites);

/// The running app's queue, reachable from code that has no `tauri::State` at hand — a photo
/// reading its own sidecar (see [`flush_photo`]). Unset in tests, where nothing is ever owed.
static APP_WRITES: std::sync::OnceLock<PhotoWrites> = std::sync::OnceLock::new();

pub(crate) fn install(writes: PhotoWrites) {
    let _ = APP_WRITES.set(writes);
}

/// Pay what the photo is owed before something reads its files, so the read sees the photo as the
/// person left it and not as it was a moment ago. A no-op when nothing is owed.
pub(crate) fn flush_photo(path: &str) {
    if let Some(writes) = APP_WRITES.get() {
        writes.flush(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const Q: Duration = Duration::from_millis(1000);

    fn recipe(engine: &str) -> Recipe {
        Recipe { engine: engine.to_string(), ..Recipe::default() }
    }
    fn save(engine: &str) -> Change {
        Change::SaveRecipe(Box::new(recipe(engine)))
    }
    fn owes_clear() -> Owed {
        Owed { develop: Some(Develop::Clear), rating: None }
    }
    fn owes_recipe(engine: &str) -> Owed {
        Owed { develop: Some(Develop::Save(Box::new(recipe(engine)))), rating: None }
    }
    fn owes_rating(stars: u8) -> Owed {
        Owed { develop: None, rating: Some(stars) }
    }

    // ── Schedule, with a clock we hold ──

    #[test]
    fn nothing_is_due_before_the_quiet_period_is_over() {
        let t0 = Instant::now();
        let mut s = Schedule::default();
        s.queue("/a.raf", Change::ClearDevelopment, t0, Q);
        assert!(s.take_due(t0 + Duration::from_millis(999)).is_empty());
        assert_eq!(s.take_due(t0 + Q), vec![("/a.raf".to_string(), owes_clear())]);
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn a_new_request_restarts_the_quiet_period() {
        let t0 = Instant::now();
        let mut s = Schedule::default();
        s.queue("/a.raf", Change::ClearDevelopment, t0, Q);
        s.queue("/a.raf", Change::ClearDevelopment, t0 + Duration::from_millis(800), Q);
        assert!(s.take_due(t0 + Duration::from_millis(1500)).is_empty());
        assert_eq!(s.take_due(t0 + Duration::from_millis(1800)).len(), 1);
    }

    #[test]
    fn the_latest_develop_request_replaces_the_earlier_one() {
        let t0 = Instant::now();
        let mut s = Schedule::default();
        s.queue("/a.raf", save("rapid"), t0, Q);
        s.queue("/a.raf", save("spektra"), t0 + Duration::from_millis(100), Q);
        s.queue("/a.raf", Change::ClearDevelopment, t0 + Duration::from_millis(200), Q);
        assert_eq!(s.len(), 1);
        assert_eq!(
            s.take_due(t0 + Duration::from_secs(5)),
            vec![("/a.raf".to_string(), owes_clear())],
            "Rapid, then Spektra, then None is one write: the clear",
        );
        s.queue("/a.raf", Change::ClearDevelopment, t0, Q);
        s.queue("/a.raf", save("rapid"), t0, Q);
        assert_eq!(s.take("/a.raf"), Some(owes_recipe("rapid")), "and None, then Rapid, is the recipe");
    }

    #[test]
    fn a_rating_and_the_develop_settings_do_not_replace_each_other() {
        let t0 = Instant::now();
        let mut s = Schedule::default();
        s.queue("/a.raf", save("rapid"), t0, Q);
        s.queue("/a.raf", Change::SetRating(2), t0, Q);
        s.queue("/a.raf", Change::SetRating(4), t0, Q);
        assert_eq!(
            s.take("/a.raf"),
            Some(Owed { develop: Some(Develop::Save(Box::new(recipe("rapid")))), rating: Some(4) }),
            "stars did not drop the recipe, and the last stars won",
        );
    }

    #[test]
    fn choosing_an_engine_again_withdraws_the_develop_part_and_keeps_the_rating() {
        let t0 = Instant::now();
        let mut s = Schedule::default();
        s.queue("/a.raf", Change::ClearDevelopment, t0, Q);
        s.queue("/a.raf", Change::SetRating(5), t0, Q);
        assert!(s.cancel_develop("/a.raf"));
        assert_eq!(s.take("/a.raf"), Some(owes_rating(5)));
        // With nothing else owed, withdrawing it empties the photo's debt altogether.
        s.queue("/b.raf", Change::ClearDevelopment, t0, Q);
        assert!(s.cancel_develop("/b.raf"));
        assert!(!s.cancel_develop("/b.raf"));
        assert!(s.take_due(t0 + Duration::from_secs(60)).is_empty());
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn photos_are_independent() {
        let t0 = Instant::now();
        let mut s = Schedule::default();
        s.queue("/a.raf", Change::ClearDevelopment, t0, Q);
        s.queue("/b.raf", Change::ClearDevelopment, t0 + Duration::from_millis(500), Q);
        s.cancel_develop("/b.raf");
        assert_eq!(s.take_due(t0 + Q), vec![("/a.raf".to_string(), owes_clear())]);
    }

    #[test]
    fn the_next_due_time_is_the_earliest() {
        let t0 = Instant::now();
        let mut s = Schedule::default();
        assert_eq!(s.next_due(), None);
        s.queue("/a.raf", Change::ClearDevelopment, t0 + Duration::from_millis(300), Q);
        s.queue("/b.raf", Change::SetRating(1), t0, Q);
        assert_eq!(s.next_due(), Some(t0 + Q));
    }

    #[test]
    fn closing_takes_everything_that_is_waiting() {
        let t0 = Instant::now();
        let mut s = Schedule::default();
        s.queue("/a.raf", Change::ClearDevelopment, t0, Q);
        s.queue("/b.raf", save("rapid"), t0, Q);
        assert_eq!(s.take_all().len(), 2);
        assert_eq!(s.len(), 0);
    }

    // ── the writer, on real (short) time ──

    #[derive(Clone, Default)]
    struct FakeDisk {
        /// What reached the disk, in order: "save:<path>:<engine>", "clear:<path>", "stars:<path>:<n>".
        log: Arc<Mutex<Vec<String>>>,
        calls: Arc<AtomicUsize>,
    }
    impl FakeDisk {
        fn note(&self, line: String) {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.log.lock().unwrap().push(line);
        }
    }
    impl Disk for FakeDisk {
        fn save_recipe(&self, path: &str, recipe: &Recipe) -> Result<(), String> {
            self.note(format!("save:{path}:{}", recipe.engine));
            Ok(())
        }
        fn clear_development(&self, path: &str) -> Result<(), String> {
            self.note(format!("clear:{path}"));
            Ok(())
        }
        fn set_rating(&self, path: &str, stars: u8) -> Result<(), String> {
            self.note(format!("stars:{path}:{stars}"));
            Ok(())
        }
    }

    fn wait_until(cond: impl Fn() -> bool) -> bool {
        for _ in 0..200 {
            if cond() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        cond()
    }

    #[test]
    fn a_queued_clear_is_written_once_after_the_quiet_period() {
        let disk = FakeDisk::default();
        let writes = PhotoWrites::start(disk.clone(), Duration::from_millis(60));
        writes.queue_clear("/photos/a.raf");
        writes.queue_clear("/photos/a.raf"); // None picked twice
        assert_eq!(disk.calls.load(Ordering::SeqCst), 0, "nothing is written at once");
        assert!(wait_until(|| disk.calls.load(Ordering::SeqCst) == 1));
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(*disk.log.lock().unwrap(), vec!["clear:/photos/a.raf".to_string()]);
        writes.close();
    }

    #[test]
    fn a_burst_of_edits_writes_the_last_recipe_once() {
        let disk = FakeDisk::default();
        let writes = PhotoWrites::start(disk.clone(), Duration::from_millis(80));
        for engine in ["rapid", "spektra", "rapid", "spektra"] {
            writes.queue_save_recipe("/photos/h.raf", recipe(engine));
            std::thread::sleep(Duration::from_millis(15));
        }
        assert_eq!(disk.calls.load(Ordering::SeqCst), 0);
        assert!(wait_until(|| disk.calls.load(Ordering::SeqCst) == 1));
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(*disk.log.lock().unwrap(), vec!["save:/photos/h.raf:spektra".to_string()]);
        writes.close();
    }

    #[test]
    fn pressing_stars_again_and_again_writes_the_last_rating_once() {
        let disk = FakeDisk::default();
        let writes = PhotoWrites::start(disk.clone(), Duration::from_millis(80));
        for stars in [1, 2, 3, 5, 4] {
            writes.queue_set_rating("/photos/r.raf", stars);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(wait_until(|| disk.calls.load(Ordering::SeqCst) == 1));
        std::thread::sleep(Duration::from_millis(150));
        assert_eq!(*disk.log.lock().unwrap(), vec!["stars:/photos/r.raf:4".to_string()]);
        writes.close();
    }

    #[test]
    fn a_rating_above_five_is_five() {
        let disk = FakeDisk::default();
        let writes = PhotoWrites::start(disk.clone(), Duration::from_secs(60));
        writes.queue_set_rating("/photos/r2.raf", 9);
        writes.flush_all();
        assert_eq!(*disk.log.lock().unwrap(), vec!["stars:/photos/r2.raf:5".to_string()]);
        writes.close();
    }

    #[test]
    fn a_recipe_and_a_rating_are_both_written() {
        let disk = FakeDisk::default();
        let writes = PhotoWrites::start(disk.clone(), Duration::from_secs(60));
        writes.queue_save_recipe("/photos/b2.raf", recipe("rapid"));
        writes.queue_set_rating("/photos/b2.raf", 3);
        writes.flush("/photos/b2.raf");
        let mut log = disk.log.lock().unwrap().clone();
        log.sort();
        assert_eq!(log, vec!["save:/photos/b2.raf:rapid".to_string(), "stars:/photos/b2.raf:3".to_string()]);
        writes.close();
    }

    #[test]
    fn choosing_an_engine_again_in_time_leaves_the_disk_alone_but_not_the_rating() {
        let disk = FakeDisk::default();
        let writes = PhotoWrites::start(disk.clone(), Duration::from_millis(120));
        writes.queue_clear("/photos/b.raf"); // None
        writes.queue_set_rating("/photos/b.raf", 2);
        std::thread::sleep(Duration::from_millis(30));
        assert!(writes.cancel("/photos/b.raf")); // back to Rapid
        assert!(wait_until(|| disk.calls.load(Ordering::SeqCst) == 1));
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(*disk.log.lock().unwrap(), vec!["stars:/photos/b.raf:2".to_string()]);
        writes.close();
    }

    #[test]
    fn closing_the_app_pays_what_was_still_waiting() {
        let disk = FakeDisk::default();
        let writes = PhotoWrites::start(disk.clone(), Duration::from_secs(60));
        writes.queue_clear("/photos/c.raf");
        writes.queue_save_recipe("/photos/d.raf", recipe("rapid"));
        writes.queue_set_rating("/photos/e0.raf", 1);
        assert_eq!(writes.pending(), 3);
        writes.flush_all();
        assert_eq!(disk.calls.load(Ordering::SeqCst), 3);
        assert_eq!(writes.pending(), 0);
        writes.close();
    }

    #[test]
    fn opening_a_photo_pays_its_own_debt_first_and_only_its_own() {
        let disk = FakeDisk::default();
        let writes = PhotoWrites::start(disk.clone(), Duration::from_secs(60));
        writes.queue_save_recipe("/photos/e.raf", recipe("spektra"));
        writes.queue_save_recipe("/photos/f.raf", recipe("rapid"));
        writes.flush("/photos/e.raf");
        assert_eq!(*disk.log.lock().unwrap(), vec!["save:/photos/e.raf:spektra".to_string()]);
        assert_eq!(writes.pending(), 1, "the other photo keeps waiting");
        writes.flush("/photos/e.raf"); // nothing left to pay: not an error, not a second write
        assert_eq!(disk.calls.load(Ordering::SeqCst), 1);
        writes.close();
    }

    #[test]
    fn a_failing_disk_does_not_stop_the_queue_nor_the_other_half_of_a_debt() {
        struct Failing(Arc<AtomicUsize>);
        impl Disk for Failing {
            fn save_recipe(&self, _: &str, _: &Recipe) -> Result<(), String> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Err("nas".into())
            }
            fn clear_development(&self, _: &str) -> Result<(), String> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Err("nas".into())
            }
            fn set_rating(&self, _: &str, _: u8) -> Result<(), String> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Err("nas".into())
            }
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let writes = PhotoWrites::start(Failing(calls.clone()), Duration::from_millis(30));
        // One photo owing both: the recipe failing must not keep the rating from being tried.
        writes.queue_save_recipe("/photos/g.raf", recipe("rapid"));
        writes.queue_set_rating("/photos/g.raf", 3);
        assert!(wait_until(|| calls.load(Ordering::SeqCst) == 2));
        writes.queue_clear("/photos/h2.raf");
        assert!(wait_until(|| calls.load(Ordering::SeqCst) == 3));
        writes.close();
    }

    #[test]
    fn a_clear_claims_the_photos_publish_generation() {
        let path = "/photos/g-claim.raf";
        let generation = crate::preview::next_publish_generation(path);
        assert!(crate::preview::publish_generation_is_current(path, generation));
        let writes = PhotoWrites::start(FakeDisk::default(), Duration::from_secs(60));
        writes.queue_clear(path);
        assert!(
            !crate::preview::publish_generation_is_current(path, generation),
            "a publish waiting from before must give up",
        );
        writes.close();
    }
}
