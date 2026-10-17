//! DeckCraft in the browser.
//!
//! Runs the same [`deckcraft_ui_egui::SlideApp`] as the desktop app through eframe's web
//! runner (wgpu: WebGPU where available, WebGL2 otherwise). Build with `trunk build --release`
//! from this directory (output in `dist/web`).
//!
//! Differences from the desktop app:
//! - no TCP control channel (browsers can't listen on sockets);
//! - File → Open and Insert → Pictures/Audio/Video use the browser file picker; bytes arrive
//!   asynchronously through `Services::inbox` (presentations open, media is inserted);
//! - Save and Export trigger a browser download;
//! - dropped files are read asynchronously by `web::WebShell` and delivered through the inbox;
//! - preferences are not persisted.
//!
//! URL query flags: `?webgl` forces the WebGL2 backend instead of WebGPU; `?blank` starts with an
//! empty presentation instead of the sample deck; `?show` starts the slide show.
//!
//! `?host=parent` is for a page of the same origin that frames the editor: it sends the presentation to
//! edit and stores what the editor saves (see `host`). `?author=<name>` is the name that signs comments.
//!
//! `?nora` opens the Nora assistant pane, connected with the page's session (`?nora=<origin>` is taken only
//! for a loopback stand-in from a loopback page, see `nora_origin`); a page that makes Nora's calls itself
//! offers `window.deckcraftNoraCall(path, body)` instead (see `web::nora`).
// //// Neoffice — the two paragraphs above are ours: upstream has neither `?host=parent`, `?author=` nor `?nora`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

// //// Neoffice — added module: the bridge to the page that frames the editor (`?host=parent`). Its rules are
// tested natively, so it is built for tests as well; only its `postMessage` half is browser code.
#[cfg(any(target_arch = "wasm32", test))]
mod host;
// //// Neoffice — added module: where the Nora pane's calls may go (`web::nora` applies it); its rules are tested natively.
#[cfg(any(target_arch = "wasm32", test))]
mod nora_origin;

#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("deckcraft-web only runs in the browser: build it with `trunk build --release` in apps/deckcraft-web");
}
