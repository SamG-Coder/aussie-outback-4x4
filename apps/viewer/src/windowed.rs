//! `viewer-windowed`: the viewer as a program with a window and no console. It is what a game
//! that is downloaded has as its program (scripts/package-release.py), so that opening it with
//! a double click opens the game and nothing else. What it says goes to the game's log, and
//! what stops it is said in a box (`game.rs`). All of it is the library's (`desktop.rs`).
//!
//! It is built this way, as a program of its own, and not made from `viewer` by changing the
//! one number in the file that says which kind it is: Windows takes a program changed after
//! it was built for one that has been tampered with, and will not start it.
#![cfg_attr(windows, windows_subsystem = "windows")]

// A page, or an app on a phone, is the library, and is not started as a program.
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    solstice_viewer::desktop::main()
}
