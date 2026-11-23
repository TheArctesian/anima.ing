//! The model launcher of Linux (task L1-6c; R-327, R-347; contract K11 Sections 9 and 9.1).
//!
//! Where `AppArmor` restricts unprivileged user namespaces, the daemon cannot make the namespaces of
//! a model server itself. A root-owned copy of this program, with a profile that allows `userns`,
//! does it: it checks the runtime program against the signed list of runtimes, joins the cgroup of
//! the server, makes the namespaces and runs the runtime. Elsewhere the daemon program runs the same
//! steps ([`launch::run`]) with no list.
//!
//! The library holds the steps and the pure parts (the list and its check, the profile text); the
//! program is `src/main.rs`.

pub mod code;
#[cfg(target_os = "linux")]
pub mod launch;
pub mod list;
pub mod profile;

/// The protocol number of the launcher. The daemon refuses a launcher with a different number
/// (`launcher_outdated`). Make it larger at each change of the arguments or of the exit codes.
pub const PROTOCOL: u32 = 1;

/// The fixed path of the installed launcher, which the profile attaches to.
pub const PATH: &str = "/usr/local/libexec/anima/anima-model-launcher";

/// The argument that makes the launcher write its protocol line and exit.
pub const PROTOCOL_ARGUMENT: &str = "--protocol";

/// The protocol line that the launcher writes for [`PROTOCOL_ARGUMENT`].
#[must_use]
pub fn protocol_line() -> String {
    format!("anima-model-launcher {PROTOCOL}")
}

/// The updater public key of the releases (R-NEW-launcherkey), as `plugins.updater.pubkey` of
/// `tauri.conf.json` holds it. A test keeps the two the same.
pub const UPDATER_PUBLIC_KEY: &str = include_str!("../updater-public-key.txt");

/// The parsed [`UPDATER_PUBLIC_KEY`]; `None` only when the built-in text is broken, which a test
/// rules out.
#[must_use]
pub fn updater_key() -> Option<updater_signature::PublicKey> {
    updater_signature::PublicKey::from_key_file(UPDATER_PUBLIC_KEY.trim()).ok()
}

#[cfg(test)]
mod tests;
