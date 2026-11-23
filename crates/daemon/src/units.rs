//! The unit files that keep the daemon running, and their install (MET-019, D-10).
//! The systemd files end in `.unit` because the name rules ban `service`; `install` writes them
//! as `anima-daemon.service`.

pub mod folders;
pub mod install;
#[cfg(target_os = "linux")]
pub mod launcher;
pub mod plan;
pub mod ready;
pub mod removal;
pub mod render;
mod sandbox;
pub mod session;
pub mod step;
pub mod token_file;
pub(crate) mod trust;
#[cfg(test)]
mod trust_cases;

pub const SYSTEMD_USER: &str = include_str!("../units/anima-daemon.user.unit");

pub const SYSTEMD_SYSTEM: &str = include_str!("../units/anima-daemon.system.unit");

pub const LAUNCH_AGENT: &str = include_str!("../units/ing.anima.daemon.plist");

/// Why the admin step of the model launcher did not complete; it cannot fail off Linux.
#[cfg(target_os = "linux")]
pub type LauncherError = launcher::offer::OfferError;
/// Why the admin step of the model launcher did not complete; it cannot fail off Linux.
#[cfg(not(target_os = "linux"))]
pub type LauncherError = std::convert::Infallible;

/// The admin step of the model launcher after `install` (L1-6c): Linux only, where `AppArmor`
/// restricts user namespaces and the launcher beside the program of `host` is not installed.
///
/// # Errors
/// [`LauncherError`] when the launcher file is missing or a command fails.
#[cfg_attr(
    not(target_os = "linux"),
    expect(clippy::unnecessary_wraps, reason = "only Linux can fail")
)]
pub fn launcher_after_install(host: &install::Host<'_>) -> Result<(), LauncherError> {
    #[cfg(target_os = "linux")]
    {
        let program = host
            .program
            .canonicalize()
            .unwrap_or_else(|_| host.program.to_owned());
        launcher::offer::install_and_say(&program, host.uid)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = host;
        Ok(())
    }
}

/// The removal of the model launcher in `anima-daemon uninstall` by the user `uid` (L1-6c):
/// Linux only, when it is installed.
///
/// # Errors
/// [`LauncherError`] when a command fails.
#[cfg_attr(
    not(target_os = "linux"),
    expect(clippy::unnecessary_wraps, reason = "only Linux can fail")
)]
pub fn launcher_after_uninstall(uid: u32) -> Result<(), LauncherError> {
    #[cfg(target_os = "linux")]
    {
        launcher::offer::uninstall_and_say(uid)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = uid;
        Ok(())
    }
}
