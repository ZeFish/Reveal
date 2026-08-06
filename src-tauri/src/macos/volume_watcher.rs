use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
use tauri::Emitter;

pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_secs(2)),
        ) {
            Ok(watcher) => watcher,
            Err(e) => {
                let _ = app.emit(
                    "app-error",
                    serde_json::json!({ "message": format!("volume watcher: {e}") }),
                );
                return;
            }
        };

        if let Err(e) = watcher.watch(Path::new("/Volumes"), RecursiveMode::NonRecursive) {
            let _ = app.emit(
                "app-error",
                serde_json::json!({ "message": format!("watch /Volumes: {e}") }),
            );
            return;
        }

        let mut previous = emit_snapshot(&app, Default::default());
        // Cheap mount-set probe; the loop only pays for the full RAW walk
        // (`find_cards` inside `emit_snapshot`) when this set changes.
        let mut last_probe = dcim_volumes();

        // macOS delivers volume mount events through `/Volumes` unreliably —
        // a card can mount with no event reaching `notify` at all, which
        // would leave the loop (and the HUD) stuck on the startup snapshot
        // forever. So we cap the wait on the channel: wake on a real event
        // for an immediate reaction, OR re-probe on this deadline regardless.
        // Polling is the correct model for `/Volumes`; events are just a
        // latency optimization on top. The probe itself is cheap (a flat
        // `readdir` of `/Volumes`), so running it every 2s costs nothing;
        // the expensive recursive RAW count only runs on an actual change.
        let poll = Duration::from_secs(2);
        loop {
            match rx.recv_timeout(poll) {
                Ok(Ok(event)) => {
                    if matches!(
                        event.kind,
                        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(_)
                    ) {
                        previous = emit_snapshot(&app, previous);
                        last_probe = dcim_volumes();
                    }
                }
                Ok(Err(e)) => {
                    let _ = app.emit(
                        "app-error",
                        serde_json::json!({ "message": format!("volume watcher: {e}") }),
                    );
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // No event in `poll`. Re-probe cheaply; only re-run the
                    // full RAW walk if the mounted-DCIM set actually changed.
                    // This is what catches mounts `notify` never told us
                    // about, without re-walking a 500-file card every 2s.
                    let probe = dcim_volumes();
                    if probe != last_probe {
                        previous = emit_snapshot(&app, previous);
                        last_probe = probe;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

fn emit_snapshot(
    app: &tauri::AppHandle,
    previous: std::collections::BTreeSet<String>,
) -> std::collections::BTreeSet<String> {
    let cards = reveal_import::find_cards();
    let current: std::collections::BTreeSet<String> = cards.iter().map(|c| c.dcim.clone()).collect();

    if current != previous {
        let _ = app.emit("cards-changed", &cards);
        for card in cards.iter().filter(|card| !previous.contains(&card.dcim)) {
            let _ = app.emit("card-mounted", card);
            crate::show_import_panel(app);
        }
        for dcim in previous.iter().filter(|dcim| !current.contains(*dcim)) {
            let _ = app.emit("card-unmounted", serde_json::json!({ "dcim": dcim }));
        }
    }

    current
}

/// Cheap probe used to short-circuit the expensive `find_cards()` walk when
/// nothing has changed at `/Volumes`. We only need the *names* of mounted
/// volumes that carry a `DCIM` directory — a few `readdir` + `is_dir` calls,
/// no recursive RAW counting. The watcher compares this set across polls;
/// `find_cards()` runs only when it differs (or on the first poll).
fn dcim_volumes() -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    let Ok(volumes) = std::fs::read_dir("/Volumes") else {
        return out;
    };
    for v in volumes.flatten() {
        let dcim = v.path().join("DCIM");
        if dcim.is_dir() {
            if let Some(s) = dcim.to_str() {
                out.insert(s.to_string());
            }
        }
    }
    out
}
