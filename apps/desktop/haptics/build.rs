//! Registers the native package without exposing plugin commands to the web view: the shell's `haptic_play` calls it.

fn main() {
    tauri_plugin::Builder::new(&[]).ios_path("ios").build();
}
