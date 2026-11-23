//! R-069, WEB-008, OD-7: the macOS trust hook end to end, in a real `WKWebView`.
//! Not libtest (`harness = false`): Tauri's event loop must own the main thread.

#[cfg(target_os = "macos")]
mod freed;
#[cfg(target_os = "macos")]
mod proof;
#[cfg(target_os = "macos")]
mod site;
/// The shell's own `web_view` source, so the proof runs the production code, not a copy.
#[cfg(target_os = "macos")]
#[path = "../../src"]
#[expect(
    dead_code,
    unused_imports,
    clippy::unused_async,
    reason = "the proof drives the web view module through open_tab and pin_tab only"
)]
mod shell {
    pub mod web_view;
}
#[cfg(target_os = "macos")]
use shell::web_view;

#[cfg(target_os = "macos")]
fn main() {
    proof::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("pinning: skipped, only macOS has the trust hook");
}
