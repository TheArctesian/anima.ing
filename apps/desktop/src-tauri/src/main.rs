#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

//! The shell is the library, so that a phone can load it (MOB-001).

fn main() {
    #[cfg(desktop)]
    anima_app_lib::run();
}
