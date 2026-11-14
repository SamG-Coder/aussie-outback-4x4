//! The game's menus and instruments on the desktop: a web view laid over the window, showing
//! the same pages the browser build shows over its canvas (`ui/`, docs/ui.md).
//!
//! The view is see-through wherever its page is, so the scene shows under the instruments. It
//! is a window of its own on top of the game's, so it is what the mouse clicks and, once
//! clicked, what the keyboard types into. The page therefore passes on what it does not use:
//! the keys held, and the mouse's movement while it has hold of the pointer. That is also what
//! a page has to do in a browser, where the game is a canvas under it, so the page is the same
//! in both.
//!
//! What passes between them is text. The game tells the page what there is to show, as JSON,
//! by calling the page's `ui.receive`; the page tells the game what was done, a line at a
//! time, in words (`key KeyW 1`, `mouse 4 -2`, `do resume`).
use std::borrow::Cow;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use winit::window::Window;
use wry::dpi::{PhysicalPosition, PhysicalSize};
use wry::http::{header::CONTENT_TYPE, Response};
use wry::{Rect, WebContext, WebView, WebViewBuilder};

/// A web view over the game's window.
pub struct Overlay {
    view: WebView,
    /// What the page has said since it was last asked.
    said: Rc<RefCell<Vec<String>>>,
}

/// What kind of file this is, for the page that asked for it.
fn kind(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    }
}

/// The file in `folder` a page's request is for, if it names one inside it.
fn file_asked_for(folder: &Path, path: &str) -> Option<PathBuf> {
    let mut file = folder.to_owned();
    let path = path.trim_start_matches('/');
    for part in (if path.is_empty() { "index.html" } else { path }).split('/') {
        // Nothing above the folder, and nothing hidden.
        if part.is_empty() || part.starts_with('.') || part.contains(['\\', ':']) {
            return None;
        }
        file.push(part);
    }
    Some(file)
}

fn bounds(window: &Window) -> Rect {
    let size = window.inner_size();
    Rect {
        position: PhysicalPosition::new(0, 0).into(),
        size: PhysicalSize::new(size.width.max(1), size.height.max(1)).into(),
    }
}

impl Overlay {
    /// Lays the pages in `folder` over `window`, starting at its `index.html`.
    pub fn new(window: &Window, folder: &Path) -> Option<Self> {
        if !folder.join("index.html").exists() {
            log::warn!(
                "no menus: there is no {}",
                folder.join("index.html").display()
            );
            return None;
        }
        let said = Rc::new(RefCell::new(Vec::new()));
        let heard = said.clone();
        let folder = folder.to_owned();
        // The web view keeps a folder of its own things, and two of them cannot share one:
        // a game that was stopped rather than closed holds on to its folder for a minute or
        // more, and the next would have no menus. So each run has its own, and clears away
        // what earlier runs left.
        let kept = std::env::temp_dir().join("aussie-outback-4x4-pages");
        if let Ok(others) = std::fs::read_dir(&kept) {
            for other in others.flatten() {
                // One still in use does not go, and is left for a later run.
                let _ = std::fs::remove_dir_all(other.path());
            }
        }
        let mut context = WebContext::new(Some(kept.join(std::process::id().to_string())));
        let view = WebViewBuilder::new_with_web_context(&mut context)
            .with_bounds(bounds(window))
            .with_transparent(true)
            .with_focused(false)
            // The pages are read from the folder as they are asked for, so a page changed on
            // disk is the changed page on its next load.
            .with_custom_protocol("ui".to_owned(), move |_, request| {
                let found = file_asked_for(&folder, request.uri().path())
                    .and_then(|file| Some((std::fs::read(&file).ok()?, kind(&file))));
                let response = match found {
                    Some((bytes, kind)) => Response::builder()
                        .header(CONTENT_TYPE, kind)
                        .header("Cache-Control", "no-store")
                        .body(Cow::Owned(bytes)),
                    None => Response::builder()
                        .status(404)
                        .body(Cow::Borrowed(&b"not found"[..])),
                };
                response.expect("a response with a known header is well formed")
            })
            .with_ipc_handler(move |message| heard.borrow_mut().push(message.into_body()))
            .with_url("ui://localhost/index.html")
            .build_as_child(window)
            .map_err(|error| log::warn!("no menus: {error}"))
            .ok()?;
        Some(Self { view, said })
    }

    /// Keeps the view the size of the window.
    pub fn resize(&self, window: &Window) {
        if let Err(error) = self.view.set_bounds(bounds(window)) {
            log::warn!("menus: {error}");
        }
    }

    /// Tells the page what there is to show: `json` is handed to its `ui.receive`.
    pub fn show(&self, json: &str) {
        // A page still loading has no `ui` yet, and is told again a moment later.
        let script = format!("window.ui&&window.ui.receive({json})");
        if let Err(error) = self.view.evaluate_script(&script) {
            log::warn!("menus: {error}");
        }
    }

    /// Runs a script in the page: for trying the pages without a hand on the mouse.
    pub fn run(&self, script: &str) {
        if let Err(error) = self.view.evaluate_script(script) {
            log::warn!("menus: {error}");
        }
    }

    /// What the page has said since this was last called, a line each.
    pub fn take(&mut self) -> Vec<String> {
        std::mem::take(&mut *self.said.borrow_mut())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_is_given_only_what_is_in_its_folder() {
        let folder = Path::new("ui");
        assert_eq!(
            file_asked_for(folder, "/"),
            Some(PathBuf::from("ui/index.html"))
        );
        assert_eq!(
            file_asked_for(folder, "/art/logo.svg"),
            Some(folder.join("art").join("logo.svg"))
        );
        assert_eq!(file_asked_for(folder, "/../secret.txt"), None);
        assert_eq!(file_asked_for(folder, "/a//b"), None);
        assert_eq!(file_asked_for(folder, "/c:\\windows"), None);
        assert_eq!(file_asked_for(folder, "/.git/config"), None);
    }
}
