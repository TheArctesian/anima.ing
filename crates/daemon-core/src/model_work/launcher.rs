//! The checks of the model launcher of Linux (Section 9.1; task L1-6c), over the text that the
//! daemon reads from the system. The daemon reads the files and runs the launcher; these
//! functions decide.

use super::cause::Containment;

/// Whether the model launcher is used: `AppArmor` is on (`/sys/module/apparmor/parameters/enabled`
/// holds `Y`) and it restricts unprivileged user namespaces
/// (`/proc/sys/kernel/apparmor_restrict_unprivileged_userns` holds `1`). A value that does not
/// read is `None`: the file is not there, so that part is off.
#[must_use]
pub fn is_used(enabled: Option<&str>, restrict: Option<&str>) -> bool {
    enabled.is_some_and(|value| value.trim() == "Y")
        && restrict.is_some_and(|value| value.trim() == "1")
}

/// Whether the profile `name` is in the text of `/sys/kernel/security/apparmor/profiles`, one
/// profile on each line as `<name> (<mode>)`.
#[must_use]
pub fn profile_loaded(profiles: &str, name: &str) -> bool {
    profiles
        .lines()
        .any(|line| line.split_once(" (").map(|(profile, _)| profile) == Some(name))
}

/// Whether a file or a folder of the launcher path is one that only root can change: owner root,
/// and no write by the group or other users. `mode` is the `st_mode` of `lstat`.
#[must_use]
pub const fn only_root_writes(owner: u32, mode: u32) -> bool {
    owner == 0 && mode & 0o022 == 0
}

/// The check of the output of `anima-model-launcher --protocol` against the line of this daemon.
///
/// # Errors
/// `launcher_outdated` when the line names a different protocol, or is not a protocol line.
pub fn protocol(output: &[u8], expected: &str) -> Result<(), Containment> {
    match std::str::from_utf8(output) {
        Ok(text) if text.trim_end_matches('\n') == expected => Ok(()),
        _ => Err(Containment::LauncherOutdated),
    }
}
