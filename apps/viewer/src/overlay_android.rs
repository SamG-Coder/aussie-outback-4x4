//! The game's menus and instruments on a phone: the system's web view laid over the game,
//! showing the same pages the desktop and the browser show (`ui/`, docs/ui.md).
//!
//! The view is the activity's to make and keep (`android/java/.../MainActivity.java`, by way
//! of `android.rs`); this is the desktop's `Overlay` (`overlay.rs`) by another road, so that
//! the game's window code is the same in both places. A phone is touched, not clicked: the
//! view is given every touch, and its page works its own buttons, as in a browser. So the
//! game has nothing to tell it of a mouse.
use std::path::Path;
use winit::window::Window;

/// The web view over the game.
pub struct Overlay;

impl Overlay {
    /// Lays the pages in `folder` over the game, starting at its `index.html`.
    pub fn new(_: &Window, folder: &Path) -> Option<Self> {
        let index = std::path::absolute(folder.join("index.html")).ok()?;
        if !index.exists() {
            log::warn!("no menus: there is no {}", index.display());
            return None;
        }
        crate::android::overlay_open(&format!("file://{}", index.display())).then_some(Self)
    }

    /// The view is kept the size of the screen by the system.
    pub fn resize(&self, _: &Window) {}

    /// Tells the page what there is to show: `json` is handed to its `ui.receive`.
    pub fn show(&self, json: &str) {
        self.run(&format!("window.ui&&window.ui.receive({json})"));
    }

    /// Nothing: a touch is the page's own to hear.
    pub fn pointer(&self, _: &str, _: f64, _: f64, _: f64) {}

    /// Runs a script in the page.
    pub fn run(&self, script: &str) {
        crate::android::overlay_run(script);
    }

    /// What the page has said since this was last called, a line each.
    pub fn take(&mut self) -> Vec<String> {
        crate::android::overlay_take()
    }
}
