//! The unit files that keep the daemon running, and their install (MET-019, D-10).
//! The systemd files end in `.unit` because the name rules ban `service`; `install` writes them
//! as `anima-daemon.service`.

pub mod folders;
pub mod install;
pub mod plan;
pub mod ready;
pub mod removal;
pub mod render;
mod sandbox;
pub mod session;
pub mod step;
pub mod token_file;
mod trust;
#[cfg(test)]
mod trust_cases;

pub const SYSTEMD_USER: &str = include_str!("../units/anima-daemon.user.unit");

pub const SYSTEMD_SYSTEM: &str = include_str!("../units/anima-daemon.system.unit");

pub const LAUNCH_AGENT: &str = include_str!("../units/ing.anima.daemon.plist");
