//! `viewer`: the viewer as a program run from a console, which is where it says what it does.
//! All of it is the library's (`desktop.rs`).

// A page, or an app on a phone, is the library, and is not started as a program.
#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn main() {}

#[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    solstice_viewer::desktop::main()
}
