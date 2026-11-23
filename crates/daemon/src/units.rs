//! The unit files that keep the daemon running, and `anima-daemon install` (MET-019, D-10): it
//! fills the running binary's path into these texts, writes the file and starts it. The
//! operator's page command and the desktop app both call it. `anima-daemon uninstall` takes the
//! user unit out again ([`removal`], HMD-056).
//! The files are in `crates/daemon/units/`; the two systemd files end in `.unit` here only because
//! the repository's name rules ban the word `service`: `install` writes them as
//! `anima-daemon.service`.

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
mod trust;
#[cfg(test)]
mod trust_cases;

/// A systemd user unit: `~/.config/systemd/user/anima-daemon.service`, run by the operator.
pub const SYSTEMD_USER: &str = include_str!("../units/anima-daemon.user.unit");

/// A systemd system unit: `/etc/systemd/system/anima-daemon.service`, run by the `anima-daemon`
/// user with its own state directory and a sandbox.
pub const SYSTEMD_SYSTEM: &str = include_str!("../units/anima-daemon.system.unit");

/// A `LaunchAgent`: `~/Library/LaunchAgents/ing.anima.daemon.plist`.
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
