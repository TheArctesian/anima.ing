//! F-10.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use serde::Serialize;

use crate::link::DeviceState;

/// The state number of a listening socket in `/proc/net/tcp`.
const LISTEN: &str = "0A";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Owner {
    /// A process of this user; the client may send its token and password.
    Own,
    /// A process of another user, or one that is not the app's own device link.
    Other,
    #[serde(rename = "none")]
    Nobody,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Listed {
    /// Linux `/proc/net`: a stranger's wildcard `::` listener also reaches IPv4 (dual stack).
    EveryUser,
    /// macOS `lsof`: a wildcard reaches its own family only, so this user's `::` never vouches for
    /// IPv4.
    ThisUser,
}

pub fn covers(bound: IpAddr, target: IpAddr, listed: Listed) -> bool {
    let (bound, target) = (bound.to_canonical(), target.to_canonical());
    match bound {
        IpAddr::V4(address) if address.is_unspecified() => target.is_ipv4(),
        IpAddr::V6(address) if address.is_unspecified() => {
            target.is_ipv6() || listed == Listed::EveryUser
        }
        address => address == target,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listener {
    pub address: IpAddr,
    pub port: u16,
    pub uid: Option<u32>,
}

pub fn proc_net_tcp(text: &str, ipv6: bool) -> Vec<Listener> {
    text.lines()
        .skip(1)
        .filter_map(|line| proc_line(line, ipv6))
        .collect()
}

fn proc_line(line: &str, ipv6: bool) -> Option<Listener> {
    let fields: Vec<&str> = line.split_ascii_whitespace().collect();
    // sl local_address rem_address st tx:rx tr:when retrnsmt uid ...
    let (local, state, uid) = (fields.get(1)?, fields.get(3)?, fields.get(7)?);
    if *state != LISTEN {
        return None;
    }
    let (address, port) = local.split_once(':')?;
    let address = if ipv6 {
        IpAddr::V6(proc_ipv6(address)?).to_canonical()
    } else {
        IpAddr::V4(Ipv4Addr::from(proc_word(address)?))
    };
    Some(Listener {
        address,
        port: u16::from_str_radix(port, 16).ok()?,
        uid: Some(uid.parse().ok()?),
    })
}

/// The kernel prints each 32-bit word in this machine's byte order.
fn proc_word(hex: &str) -> Option<[u8; 4]> {
    if hex.len() != 8 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    Some(u32::from_str_radix(hex, 16).ok()?.to_ne_bytes())
}

fn proc_ipv6(hex: &str) -> Option<Ipv6Addr> {
    if hex.len() != 32 || !hex.is_ascii() {
        return None;
    }
    let mut octets = [0_u8; 16];
    for (word, chunk) in octets.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        *chunk = proc_word(hex.get(word * 8..word * 8 + 8)?)?;
    }
    Some(Ipv6Addr::from(octets))
}

/// In `lsof -F nt` output a `t` line gives the family of the `n` lines after it, and `*` is that
/// family's unspecified address. `lsof` gives no owner; the caller asked for this user's processes
/// only.
pub fn lsof_names(text: &str) -> Vec<Listener> {
    let mut family: Option<IpAddr> = None;
    let mut found = Vec::new();
    for line in text.lines() {
        if let Some(kind) = line.strip_prefix('t') {
            family = match kind {
                "IPv4" => Some(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
                "IPv6" => Some(IpAddr::V6(Ipv6Addr::UNSPECIFIED)),
                _ => None,
            };
        } else if let Some(name) = line.strip_prefix('n') {
            found.extend(lsof_name(name, family));
        } else if line.starts_with('f') || line.starts_with('p') {
            family = None;
        }
    }
    found
}

fn lsof_name(name: &str, wildcard: Option<IpAddr>) -> Option<Listener> {
    let (address, port) = name.rsplit_once(':')?;
    let address = if address == "*" {
        wildcard?
    } else {
        address
            .strip_prefix('[')
            .and_then(|inner| inner.strip_suffix(']'))
            .unwrap_or(address)
            .parse()
            .ok()?
    };
    Some(Listener {
        address,
        port: port.parse().ok()?,
        uid: None,
    })
}

/// At least one listener covers the target, and each one that covers it is this user's.
pub fn only_own(
    listeners: &[Listener],
    listed: Listed,
    (target, port): (IpAddr, u16),
    uid: u32,
) -> bool {
    let mut covering = listeners
        .iter()
        .filter(|listener| listener.port == port && covers(listener.address, target, listed))
        .peekable();
    covering.peek().is_some()
        && covering.all(|listener| listener.uid.is_none_or(|owner| owner == uid))
}

/// The device link's saved port counts as own only while the app's own link serves it (F-10).
pub fn link_permits(link: &DeviceState, port: u16) -> bool {
    link.port != Some(port) || link.serves(port)
}

/// A listener of this user that did not accept in time still counts as own.
pub fn verdict(accepts: bool, own: bool) -> Owner {
    match (accepts, own) {
        (_, true) => Owner::Own,
        (true, false) => Owner::Other,
        (false, false) => Owner::Nobody,
    }
}
