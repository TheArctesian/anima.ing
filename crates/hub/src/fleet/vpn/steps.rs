//! Linux VPN join steps (vpn.md Section 5, VPN-001, VPN-003).
//! The key goes through a hidden prompt into a root-only file, never a command line, a history or a step.

use ipnet::IpNet;

use super::LOGIN_SERVER;

const READ_KEY: &str = r#"sudo bash -c 'umask 077; read -rsp "Pre-auth key: " K; echo; printf "%s" "$K" > /run/anima-preauth'"#;
const REMOVE_KEY: &str = "sudo rm -f /run/anima-preauth";

/// The caller checks `site` (`[a-z0-9-]{1,32}`); the key carries the tags, so no `--advertise-tags`.
pub(super) fn router(site: &str, subnet: IpNet) -> Vec<String> {
    steps(&format!(
        "--advertise-routes={} --hostname {site}-router",
        subnet.trunc()
    ))
}

pub(super) fn computer() -> Vec<String> {
    steps("--accept-routes")
}

fn steps(flags: &str) -> Vec<String> {
    vec![
        READ_KEY.to_owned(),
        format!(
            "sudo tailscale up --login-server {LOGIN_SERVER} --auth-key=file:/run/anima-preauth {flags}"
        ),
        REMOVE_KEY.to_owned(),
    ]
}
