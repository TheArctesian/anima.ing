//! Which addresses the hub may connect to (R-030, R-032, P3-003): every outbound connection
//! goes through this one rule, and [`connect`] reaches a computer of the link (HMD-048).
//! Private and VPN ranges stay allowed, so the rule does not decide which host there is reached.

mod link_port;
mod reached;

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
};

use ipnet::IpNet;

pub use link_port::LinkPort;
pub use reached::{Reached, connect, on_link, preview, provider_login};

/// The default refuses loopback, link-local, cloud metadata and the other addresses the hub
/// never touches, in each IPv6 form.
#[derive(Clone, Debug, Default)]
pub struct AddressPolicy {
    extra_deny: Vec<IpNet>,
    allow_loopback: bool,
    link_dial: Option<Arc<Path>>,
}

impl AddressPolicy {
    #[must_use]
    pub fn new(extra_deny: Vec<IpNet>) -> Self {
        Self {
            extra_deny,
            allow_loopback: false,
            link_dial: None,
        }
    }

    /// [`connect`] then reaches a link address through `dial` (HMD-048); `None` changes nothing.
    #[must_use]
    pub fn reaching_link(mut self, dial: Option<PathBuf>) -> Self {
        self.link_dial = dial.map(Arc::from);
        self
    }

    fn link_dial(&self) -> Option<&Path> {
        self.link_dial.as_deref()
    }

    /// Also allows loopback as written, not in an IPv6 form, for tests; the deny list still wins.
    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub fn allowing_loopback(mut self) -> Self {
        self.allow_loopback = true;
        self
    }

    #[cfg(any(test, feature = "test-support"))]
    #[must_use]
    pub(crate) const fn allows_loopback(&self) -> bool {
        self.allow_loopback
    }

    #[must_use]
    pub fn permits(&self, ip: IpAddr) -> bool {
        let canonical = canonical(ip);
        if self
            .extra_deny
            .iter()
            .any(|net| net.contains(&ip) || net.contains(&canonical))
        {
            return false;
        }
        if self.allow_loopback && ip.is_loopback() {
            return true;
        }
        !always_refused(canonical) && !local_use_nat64(ip)
    }
}

/// Holds no host name or address (LOG-007).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TargetError {
    #[error("the host name does not resolve")]
    Unresolved,
    #[error("the host resolves to an address the hub may not reach")]
    Refused,
}

/// Every canonical address `host` resolves to, with `port`, when `policy` permits every one
/// (D-1, R-032). The caller connects only to a returned address and never resolves the name
/// again, so a second lookup cannot point the connection elsewhere.
///
/// # Errors
/// [`TargetError::Unresolved`] for no address; [`TargetError::Refused`] when one is refused.
pub async fn resolve_checked(
    host: &str,
    port: u16,
    policy: &AddressPolicy,
) -> Result<Vec<SocketAddr>, TargetError> {
    let addresses = lookup(host, &HashMap::new())
        .await?
        .into_iter()
        .map(|ip| SocketAddr::new(ip.to_canonical(), port))
        .collect();
    every_permitted(policy, addresses)
}

/// The addresses `host` names, unchecked: every caller checks each against the policy before
/// it connects. Bracketed text other than one IPv6 literal never reaches DNS; `overrides`
/// (lower-case keys) replaces DNS for the checks tests.
///
/// # Errors
/// [`TargetError::Unresolved`] when `host` names no address.
pub(crate) async fn lookup(
    host: &str,
    overrides: &HashMap<String, Vec<IpAddr>>,
) -> Result<Vec<IpAddr>, TargetError> {
    if let Some(inside) = host.strip_prefix('[') {
        let ip = inside
            .strip_suffix(']')
            .and_then(|literal| literal.parse::<Ipv6Addr>().ok())
            .ok_or(TargetError::Unresolved)?;
        return Ok(vec![IpAddr::V6(ip)]);
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![ip]);
    }
    let found: Vec<IpAddr> = match overrides.get(&host.to_ascii_lowercase()) {
        Some(ips) => ips.clone(),
        None => tokio::net::lookup_host((host, 0))
            .await
            .map_err(|_| TargetError::Unresolved)?
            .map(|address| address.ip())
            .collect(),
    };
    if found.is_empty() {
        Err(TargetError::Unresolved)
    } else {
        Ok(found)
    }
}

fn every_permitted(
    policy: &AddressPolicy,
    addresses: Vec<SocketAddr>,
) -> Result<Vec<SocketAddr>, TargetError> {
    if addresses.is_empty() {
        return Err(TargetError::Unresolved);
    }
    if addresses
        .iter()
        .any(|address| !policy.permits(address.ip()))
    {
        return Err(TargetError::Refused);
    }
    Ok(addresses)
}

fn embedded_v4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let o = ip.octets();
    let v4 = |at: usize| Ipv4Addr::new(o[at], o[at + 1], o[at + 2], o[at + 3]);
    let mapped = o[..10] == [0; 10] && o[10] == 0xff && o[11] == 0xff;
    // ::a.b.c.d, but neither :: nor ::1, which stay IPv6 addresses.
    let compatible = o[..12] == [0; 12] && o[12..] != [0; 4] && o[12..] != [0, 0, 0, 1];
    let nat64 = o[..12] == [0, 0x64, 0xff, 0x9b, 0, 0, 0, 0, 0, 0, 0, 0];
    // ::ffff:0:a.b.c.d (IPv4-translated, SIIT)
    let translated = o[..8] == [0; 8] && o[8..12] == [0xff, 0xff, 0, 0];
    if mapped || compatible || nat64 || translated {
        Some(v4(12))
    } else if o[..6] == [0, 0x64, 0xff, 0x9b, 0, 1] {
        // 64:ff9b:1::/48 (local-use NAT64): the address sits around the reserved octet 8.
        Some(Ipv4Addr::new(o[6], o[7], o[9], o[10]))
    } else if o[0] == 0x20 && o[1] == 0x02 {
        Some(v4(2)) // 2002::/16 (6to4)
    } else {
        None
    }
}

fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => embedded_v4(v6).map_or(IpAddr::V6(v6), IpAddr::V4),
        v4 @ IpAddr::V4(_) => v4,
    }
}

/// `64:ff9b:1::/48`, whatever it embeds: the prefix is local-use, so a translator may place
/// an IPv4 address in it by a rule this policy cannot know (CHK-012).
fn local_use_nat64(ip: IpAddr) -> bool {
    matches!(ip, IpAddr::V6(v6) if v6.segments()[..3] == [0x64, 0xff9b, 1])
}

fn always_refused(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_loopback()
                || v4.is_unspecified()
                || o[0] == 0
                || v4.is_link_local()
                || v4.is_multicast()
                || o[0] >= 240
                || o[..3] == [100, 100, 100] // Tailscale 100.100.100.0/24 (quad-100)
                || o[..3] == [192, 0, 0] // 192.0.0.0/24 IETF protocol assignments
                || v4 == Ipv4Addr::new(168, 63, 129, 16) // Azure wire server
        }
        IpAddr::V6(v6) => {
            let [first, second, ..] = v6.segments();
            v6.is_loopback()
                || (first == 0x2001 && second == 0) // 2001::/32 Teredo
                || first & 0xffc0 == 0xfec0 // fec0::/10 site-local
                || v6.is_unspecified()
                || v6.is_multicast()
                || first & 0xffc0 == 0xfe80
                || v6 == Ipv6Addr::new(0xfd00, 0x0ec2, 0, 0, 0, 0, 0, 0x0254)
        }
    }
}

#[cfg(test)]
pub(crate) mod tests;
