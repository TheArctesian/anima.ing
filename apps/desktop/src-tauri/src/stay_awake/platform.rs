use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Mac,
    Linux,
}

/// The macOS programs are absolute paths, so `PATH` cannot swap them; the Linux ones run with this user's rights only.
#[derive(Debug, Clone)]
pub struct Platform {
    pub os: Os,
    /// Holds the idle-sleep assertion on macOS (AWK-002).
    pub caffeinate: String,
    pub inhibit: String,
    pub pmset: String,
    /// Runs `pmset` as root through the set-up step's rule, never with a password.
    pub sudo: String,
    /// Asks macOS for an administrator, for the set-up step only.
    pub osascript: String,
    /// Runs the watcher that restores the sleep setting when the app ends (AWK-007).
    pub shell: String,
    pub rule: PathBuf,
    /// `XDG_CURRENT_DESKTOP`: some desktops decide the lid themselves.
    pub desktop: String,
    /// The process the holds follow: they end when it ends.
    pub followed: u32,
    pub user: u32,
}

impl Platform {
    #[must_use]
    pub fn here() -> Self {
        Self {
            os: if cfg!(target_os = "macos") {
                Os::Mac
            } else {
                Os::Linux
            },
            caffeinate: "/usr/bin/caffeinate".into(),
            inhibit: "systemd-inhibit".into(),
            pmset: "/usr/bin/pmset".into(),
            sudo: "/usr/bin/sudo".into(),
            osascript: "/usr/bin/osascript".into(),
            shell: "/bin/sh".into(),
            rule: PathBuf::from("/private/etc/sudoers.d/anima-stay-awake"),
            desktop: std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default(),
            followed: std::process::id(),
            user: nix::unistd::getuid().as_raw(),
        }
    }
}
