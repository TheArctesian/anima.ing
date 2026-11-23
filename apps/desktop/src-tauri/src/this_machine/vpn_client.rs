//! Never an interface address: Headscale, Tailscale and `NetBird` all hand out addresses in
//! 100.64.0.0/10, so the range does not say which network an address belongs to (ADR-005, R-060).

use std::{
    ffi::OsStr,
    net::{IpAddr, Ipv4Addr},
    path::Path,
    time::Duration,
};

use serde::Deserialize;
use url::Url;

use super::{OperatingSystem, address::is_vpn_address};
use crate::os_program::{self, AfterLimit, Limit, Stdout};

pub(super) const CLIENT_PATIENCE: Duration = Duration::from_secs(1);
const MAX_OUTPUT: u64 = 16 * 1024;

/// Absolute paths, so `PATH` cannot swap it.
const MACOS_CLIENTS: [&str; 3] = [
    "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
    "/opt/homebrew/bin/tailscale",
    "/usr/local/bin/tailscale",
];
const LINUX_CLIENTS: [&str; 3] = [
    "/usr/bin/tailscale",
    "/usr/sbin/tailscale",
    "/usr/local/bin/tailscale",
];

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Prefs {
    #[serde(rename = "ControlURL")]
    control_url: String,
    #[serde(default)]
    logged_out: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Status {
    backend_state: String,
    #[serde(rename = "Self")]
    own_node: Option<Node>,
}

#[derive(Deserialize)]
struct Node {
    #[serde(rename = "TailscaleIPs", default)]
    addresses: Vec<IpAddr>,
}

pub(super) fn client_program(os: OperatingSystem) -> Option<&'static str> {
    let places = match os {
        OperatingSystem::Macos => MACOS_CLIENTS,
        OperatingSystem::Linux => LINUX_CLIENTS,
        OperatingSystem::Windows => return None,
    };
    places.into_iter().find(|place| Path::new(place).is_file())
}

pub(super) fn own_address(
    program: &str,
    login_server: &str,
    patience: Duration,
) -> Option<Ipv4Addr> {
    let prefs = ask(program, &["debug", "prefs"], patience)?;
    if !signed_in_to(&prefs, login_server) {
        return None;
    }
    let status = ask(program, &["status", "--json", "--peers=false"], patience)?;
    running_address(&status)
}

/// Origins are compared, so case, a default port or a trailing slash make no difference.
pub(super) fn signed_in_to(prefs: &[u8], login_server: &str) -> bool {
    let Ok(prefs) = serde_json::from_slice::<Prefs>(prefs) else {
        return false;
    };
    let (Ok(theirs), Ok(ours)) = (Url::parse(&prefs.control_url), Url::parse(login_server)) else {
        return false;
    };
    !prefs.logged_out && theirs.origin() == ours.origin()
}

pub(super) fn running_address(status: &[u8]) -> Option<Ipv4Addr> {
    let status = serde_json::from_slice::<Status>(status).ok()?;
    if status.backend_state != "Running" {
        return None;
    }
    status
        .own_node?
        .addresses
        .into_iter()
        .find_map(|ip| match ip {
            IpAddr::V4(v4) if is_vpn_address(v4) => Some(v4),
            _ => None,
        })
}

fn ask(program: &str, args: &[&str], patience: Duration) -> Option<Vec<u8>> {
    let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
    let limit = Limit {
        patience,
        after: AfterLimit::Kill,
    };
    let output = Stdout::Capture {
        max_bytes: MAX_OUTPUT,
    };
    os_program::run(program, &args, limit, output).ok()
}
