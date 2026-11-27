//! Managed images supply their toolchain environment; user profiles cannot replace it.
//! This selects startup behavior, not caller authorization or a credential grant.
const NAMES: &[&str] = &[
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "PATH",
    "TMPDIR",
    "LANG",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "DISABLE_AUTOUPDATER",
    "FORCE_AUTOUPDATE_PLUGINS",
];

pub(super) fn environment(daemon: &[(String, String)]) -> Option<Vec<(String, String)>> {
    if !daemon.iter().any(|(name, value)| {
        name == "ANIMA_MANAGED_OWNER_SOCKET" && value == "/run/anima/owner.sock"
    }) {
        return None;
    }
    let secrets = super::secrets_of(daemon);
    Some(
        daemon
            .iter()
            .filter(|(name, value)| NAMES.contains(&name.as_str()) && !secrets.contains(value))
            .cloned()
            .collect(),
    )
}
