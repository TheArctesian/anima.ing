//! Stand-ins replace `sudo`, `pmset`, `osascript` and `systemd-inhibit`: no test changes a power setting.
//! One macOS test takes a real `caffeinate` assertion, which needs no rights.

mod controls;
mod holds;
mod levels;
mod rig;
mod set_up;
mod steps;
