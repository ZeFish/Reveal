//! The detached develop panel goes away with the main window and comes back with it.
//!
//! The panel used to be a child window of the main one, which made it follow every drag (see
//! `paletteManager.js`). Now it is free to be moved, but it still has to leave when the main
//! window leaves — minimised, or closed (Reveal hides the main window rather than closing it)
//! — and return when the main window does. The panel is only brought back if this module is
//! what put it away: a panel the interface hid itself (back in the grid, not detached) stays hidden.

use tauri::Manager;

/// What to do with the panel after looking at the main window.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Nothing,
    HidePanel,
    ShowPanel,
}

/// The memory of the watcher between two looks.
#[derive(Default)]
pub(crate) struct Watch {
    main_was_away: bool,
    we_hid_the_panel: bool,
}

impl Watch {
    /// `main_away`: the main window is minimised or hidden. `panel_shown`: the panel is on screen.
    pub(crate) fn look(&mut self, main_away: bool, panel_shown: bool) -> Step {
        let step = if main_away && !self.main_was_away && panel_shown {
            self.we_hid_the_panel = true;
            Step::HidePanel
        } else if !main_away && self.main_was_away && self.we_hid_the_panel {
            self.we_hid_the_panel = false;
            Step::ShowPanel
        } else {
            Step::Nothing
        };
        self.main_was_away = main_away;
        step
    }
}

const LOOK_EVERY_MS: u64 = 250;

/// Watch the main window for as long as the app runs. Minimising has no event of its own in
/// Tauri, so the window is looked at a few times a second (two cheap calls).
pub(crate) fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut watch = Watch::default();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(LOOK_EVERY_MS));
            let (Some(main), Some(panel)) = (
                app.get_webview_window("main"),
                app.get_webview_window("develop-panel"),
            ) else {
                // No panel to keep company: forget what we did to the last one.
                watch = Watch::default();
                continue;
            };
            let main_away = !main.is_visible().unwrap_or(true) || main.is_minimized().unwrap_or(false);
            let panel_shown = panel.is_visible().unwrap_or(false);
            match watch.look(main_away, panel_shown) {
                Step::HidePanel => {
                    let _ = panel.hide();
                }
                Step::ShowPanel => {
                    let _ = panel.show();
                }
                Step::Nothing => {}
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_leaves_with_the_main_window_and_comes_back_with_it() {
        let mut w = Watch::default();
        assert_eq!(w.look(false, true), Step::Nothing);
        assert_eq!(w.look(true, true), Step::HidePanel);
        assert_eq!(w.look(true, false), Step::Nothing); // still away, panel already gone
        assert_eq!(w.look(false, false), Step::ShowPanel);
        assert_eq!(w.look(false, true), Step::Nothing);
    }

    #[test]
    fn a_panel_the_interface_had_hidden_stays_hidden() {
        let mut w = Watch::default();
        assert_eq!(w.look(false, false), Step::Nothing); // back in the grid: panel hidden by the app
        assert_eq!(w.look(true, false), Step::Nothing); // main minimised
        assert_eq!(w.look(false, false), Step::Nothing); // main back: not ours to show
    }

    #[test]
    fn the_panel_is_not_brought_back_after_the_app_chose_to_hide_it_meanwhile() {
        let mut w = Watch::default();
        assert_eq!(w.look(true, true), Step::HidePanel);
        // The person comes back, and the interface shows the panel by itself before we look.
        assert_eq!(w.look(false, true), Step::ShowPanel); // harmless: showing a shown window
    }
}
