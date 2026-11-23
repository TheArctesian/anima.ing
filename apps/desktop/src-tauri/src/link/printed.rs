//! Read without trust (2.6). An invite is a secret; nothing here keeps or quotes one.

use std::{net::SocketAddr, path::PathBuf};

use zeroize::Zeroizing;

use super::LinkError;

const INVITE_PREFIX: &str = "anima-invite.1.";
const MOST_INVITE: usize = 4096;
/// As `link/cmd/anima-link/join.go` prints it.
pub const JOINED: &str = "This device has joined the hub. Its key fingerprint is ";
/// The `msg` of the ready line that `link/cmd/anima-link/device.go` logs through `slog`'s text
/// handler.
pub const READY: &str = "msg=\"the device link is ready\"";
const NO_ANSWER: &str = "the hub did not answer";
const REFUSED: &str = "the hub refused this device";
const PORT_TAKEN: &str = "address already in use";

pub fn invite(pasted: &str) -> Result<Zeroizing<String>, LinkError> {
    let invite = pasted.trim_matches(|c: char| c.is_ascii_whitespace());
    let fits = invite.len() <= MOST_INVITE
        && invite.len() > INVITE_PREFIX.len()
        && invite.starts_with(INVITE_PREFIX)
        && invite
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'));
    if !fits {
        return Err(LinkError::BadInvite);
    }
    Ok(Zeroizing::new(format!("{invite}\n")))
}

pub fn fingerprint(printed: &str) -> Option<String> {
    let rest = printed.lines().find_map(|line| line.strip_prefix(JOINED))?;
    let fingerprint = rest.strip_suffix('.')?;
    let fits = (8..=64).contains(&fingerprint.len())
        && fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit());
    fits.then(|| fingerprint.to_owned())
}

pub fn join_failure(said: &str) -> LinkError {
    if said.contains(NO_ANSWER) {
        LinkError::NoAnswer
    } else if said.contains(REFUSED) {
        LinkError::Refused
    } else {
        LinkError::Failed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceLine {
    Ready(u16),
    PortTaken,
}

pub fn device_line(line: &str) -> Option<DeviceLine> {
    if line.contains(PORT_TAKEN) {
        return Some(DeviceLine::PortTaken);
    }
    if !line.contains(READY) {
        return None;
    }
    let local = line
        .split_ascii_whitespace()
        .find_map(|field| field.strip_prefix("local="))?;
    let address: SocketAddr = local.trim_matches('"').parse().ok()?;
    (address.ip().is_loopback() && address.port() != 0).then_some(DeviceLine::Ready(address.port()))
}

pub fn link_local(printed: &[u8]) -> Option<u16> {
    let status: serde_json::Value = serde_json::from_slice(printed).ok()?;
    if status.get("role")?.as_str()? != "device" {
        return None;
    }
    let address: SocketAddr = status.get("local")?.as_str()?.parse().ok()?;
    (address.ip().is_loopback() && address.port() != 0).then_some(address.port())
}

pub fn unit_installed(printed: &[u8]) -> bool {
    serde_json::from_slice::<serde_json::Value>(printed)
        .ok()
        .and_then(|shown| shown.get("installed")?.as_bool())
        .unwrap_or(false)
}

pub fn unit_file(printed: &[u8]) -> Option<PathBuf> {
    let shown: serde_json::Value = serde_json::from_slice(printed).ok()?;
    let file = PathBuf::from(shown.get("file")?.as_str()?);
    (unit_installed(printed) && file.is_absolute()).then_some(file)
}

/// The folder must be one whole argument: a `<string>` of a launch agent or a quoted word of a
/// systemd unit.
pub fn unit_runs_folder(unit: &str, folder: &str) -> bool {
    let task_folder = folder
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&#34;")
        .replace('\'', "&#39;")
        .replace('\r', "&#xD;")
        .replace('\n', "&#xA;")
        .replace('\t', "&#x9;");
    unit.contains(&format!("<string>{folder}</string>"))
        || unit.contains(&format!("\"{folder}\""))
        || unit.contains(&format!("&#34;{task_folder}&#34;"))
}

pub fn link_path(printed: &[u8]) -> Option<(String, f64, Option<String>)> {
    let status: serde_json::Value = serde_json::from_slice(printed).ok()?;
    if status.get("role")?.as_str()? != "device" {
        return None;
    }
    let path = status.get("path")?.as_str()?;
    let latency = status.get("latencyMs")?.as_f64()?;
    let known = matches!(path, "direct" | "relayed" | "none");
    let key = status
        .get("publicKey")
        .and_then(serde_json::Value::as_str)
        .filter(|key| key.starts_with("nodekey:") && key.len() == "nodekey:".len() + 64)
        .map(str::to_owned);
    (known && latency.is_finite() && latency >= 0.0).then(|| (path.to_owned(), latency, key))
}
