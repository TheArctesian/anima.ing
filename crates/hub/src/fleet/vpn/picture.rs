//! The picture from one status reply (VPN-009 to VPN-011); pure, the reader brings the inputs.

use std::{
    collections::HashSet,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

use chrono::{DateTime, Duration, Utc};
use ipnet::IpNet;

use super::{
    Error,
    broker::BrokerView,
    model::{BrokerState, KeyState, Picture, Role, RouteState, SitePath, VpnNode, VpnPath},
    peers::{Peer, Status, shown},
    waiting,
};
use crate::{
    fleet::devices::Device,
    ids::{DeviceId, SiteId},
};

const EXPIRING_DAYS: i64 = 30;
/// Reads that keep a node's last known path after its traffic stops (BD-13).
const KEPT_READS: u8 = 3;

/// The tailnet's own IPv4 range (Headscale `100.64.0.0/10`).
const TAILNET_V4: IpNet = IpNet::new_assert(IpAddr::V4(Ipv4Addr::new(100, 64, 0, 0)), 10);
/// The tailnet's own IPv6 range (Headscale `fd7a:115c:a1e0::/48`).
const TAILNET_V6: IpNet = IpNet::new_assert(
    IpAddr::V6(Ipv6Addr::new(0xfd7a, 0x115c, 0xa1e0, 0, 0, 0, 0, 0)),
    48,
);

/// The hub's container networks (`deploy/compose.yaml`); the host firewall drops this range from the VPN.
const HUB_NETWORKS: IpNet = IpNet::new_assert(IpAddr::V4(Ipv4Addr::new(172, 30, 0, 0)), 16);

/// A router could not serve a subnet that overlaps the tailnet range apart from the tailnet's own addresses.
pub(super) fn on_tailnet(subnet: IpNet) -> bool {
    [TAILNET_V4, TAILNET_V6]
        .iter()
        .any(|range| overlaps(*range, subnet))
}

/// A router serving such a subnet could send from addresses the hub's host trusts, so the site gets no router (VP-13).
pub(super) fn on_hub_networks(subnet: IpNet) -> bool {
    overlaps(HUB_NETWORKS, subnet)
}

fn overlaps(range: IpNet, subnet: IpNet) -> bool {
    range.contains(&subnet) || subnet.contains(&range)
}

fn is_router_site(subnet: IpNet) -> bool {
    !on_tailnet(subnet) && !on_hub_networks(subnet)
}

/// `sites` is in site name order.
pub(super) struct Inputs<'a> {
    pub status: &'a Status,
    pub sites: &'a [(SiteId, IpNet)],
    pub devices: &'a [Device],
    /// The router devices with a Tailscale ping check that is not paused.
    pub ping_checked: &'a HashSet<DeviceId>,
    pub now: DateTime<Utc>,
    /// The broker's answer of this read; `broker` is its node list when it is `Ready`.
    pub broker_state: BrokerState,
    pub broker: Option<&'a BrokerView>,
}

/// `Unreadable` when the hub's own node has no IPv4 address, as it is not on the VPN.
pub(super) fn build(inputs: &Inputs<'_>) -> Result<Picture, Error> {
    let hub = node(&inputs.status.own, inputs).ok_or(Error::Unreadable)?;
    let mut nodes: Vec<VpnNode> = peers(inputs.status)
        .filter_map(|peer| node(peer, inputs))
        .collect();
    nodes.sort_by(|a, b| (&a.name, a.address).cmp(&(&b.name, b.address)));
    let sites: Vec<SitePath> = inputs
        .sites
        .iter()
        .filter(|&&(_, subnet)| is_router_site(subnet))
        .map(|&(site, subnet)| {
            let mut path = site_path(site, subnet, &nodes, inputs.ping_checked);
            path.waiting_router = inputs
                .broker
                .and_then(|view| waiting::router(view, subnet, inputs));
            path
        })
        .collect();
    for router in sites.iter().filter_map(|site| site.router) {
        nodes[router].in_anima = true;
    }
    Ok(Picture {
        read_at: inputs.now,
        fresh: true,
        hub,
        sites,
        nodes,
        broker: inputs.broker_state,
        roster: inputs.broker.cloned(),
    })
}

pub(super) fn router_devices(
    status: &Status,
    sites: &[(SiteId, IpNet)],
    devices: &[Device],
) -> Vec<DeviceId> {
    let mut found: Vec<DeviceId> = peers(status)
        .filter(|peer| role(&peer.tags) == Role::Router)
        .filter(|peer| {
            sites
                .iter()
                .filter(|&&(_, subnet)| is_router_site(subnet))
                .any(|&(_, subnet)| serves(&peer.primary_routes, subnet))
        })
        .filter_map(|peer| device_of(peer, devices).map(|(id, _)| id))
        .collect();
    found.sort_unstable_by_key(|id| uuid::Uuid::from(*id));
    found.dedup();
    found
}

/// An Unknown path keeps the last known one for a few reads; an offline node keeps nothing (BD-13, VPN-009).
pub(super) fn carry(previous: &Picture, mut next: Picture) -> Picture {
    let earlier = |key: &str| {
        std::iter::once(&previous.hub)
            .chain(&previous.nodes)
            .find(|node| node.key == key)
    };
    for node in std::iter::once(&mut next.hub).chain(&mut next.nodes) {
        if node.path != VpnPath::Unknown || !node.online {
            continue;
        }
        let Some(before) = earlier(&node.key) else {
            continue;
        };
        node.unknown_reads = before.unknown_reads.saturating_add(1);
        if before.path != VpnPath::Unknown && before.unknown_reads < KEPT_READS {
            node.path = before.path;
            node.relay_region.clone_from(&before.relay_region);
        }
    }
    next
}

fn peers(status: &Status) -> impl Iterator<Item = &Peer> {
    status.peer.iter().flat_map(|peers| peers.values())
}

fn node(peer: &Peer, inputs: &Inputs<'_>) -> Option<VpnNode> {
    let address = first_ipv4(peer)?;
    let role = role(&peer.tags);
    let device = device_of(peer, inputs.devices);
    let (path, relay_region) = path(peer);
    Some(VpnNode {
        key: peer.public_key.clone(),
        name: name(peer),
        address,
        os: shown(&peer.os, 32),
        role,
        tags: peer.tags.iter().map(|tag| shown(tag, 63)).collect(),
        routes: peer.primary_routes.iter().map(IpNet::trunc).collect(),
        waiting_routes: inputs
            .broker
            .map_or_default(|view| waiting::routes(view, address)),
        online: peer.online,
        last_seen: peer.last_seen,
        key_expiry: peer.key_expiry,
        key_state: key_state(&peer.tags, peer.key_expiry, inputs.now),
        // A router is in Anima when it serves a site; `build` sets that after the sites.
        in_anima: role == Role::Hub || (role != Role::Router && device.is_some()),
        device,
        path,
        relay_region,
        unknown_reads: 0,
    })
}

fn name(peer: &Peer) -> String {
    match peer.dns_name.split('.').next() {
        Some(label) if !label.is_empty() => shown(label, 63),
        _ => shown(&peer.host_name, 63),
    }
}

fn first_ipv4(peer: &Peer) -> Option<Ipv4Addr> {
    peer.tailscale_ips.iter().find_map(|ip| match ip {
        IpAddr::V4(v4) => Some(*v4),
        IpAddr::V6(_) => None,
    })
}

pub(super) fn role(tags: &[String]) -> Role {
    let has = |wanted: &str| tags.iter().any(|tag| tag == wanted);
    if has("tag:hub") {
        Role::Hub
    } else if has("tag:router") {
        Role::Router
    } else if has("tag:daemon") {
        Role::Daemon
    } else if tags.is_empty() {
        Role::Client
    } else {
        Role::Other
    }
}

fn path(peer: &Peer) -> (VpnPath, Option<String>) {
    if !peer.online {
        (VpnPath::Unknown, None)
    } else if !peer.peer_relay.is_empty() {
        (VpnPath::PeerRelay, None)
    } else if peer.active && !peer.cur_addr.is_empty() {
        (VpnPath::Direct, None)
    } else if peer.active {
        let region = (!peer.relay.is_empty()).then(|| shown(&peer.relay, 16));
        (VpnPath::Relayed, region)
    } else {
        (VpnPath::Unknown, None)
    }
}

/// VPN-010: a tagged key must not expire; an untagged one warns 30 days ahead.
pub(super) fn key_state(
    tags: &[String],
    expiry: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> KeyState {
    match expiry {
        Some(_) if !tags.is_empty() => KeyState::TaggedWithExpiry,
        None => KeyState::Never,
        Some(expiry) if expiry <= now => KeyState::Expired,
        Some(expiry) if expiry <= now + Duration::days(EXPIRING_DAYS) => KeyState::Expiring,
        Some(_) => KeyState::Valid,
    }
}

/// The host name the node chose is never used, so a node cannot hide behind a device's name (BD-14).
fn device_of(peer: &Peer, devices: &[Device]) -> Option<(DeviceId, String)> {
    device_by(first_ipv4(peer), &peer.dns_name, devices)
}

pub(super) fn device_by(
    address: Option<Ipv4Addr>,
    dns_name: &str,
    devices: &[Device],
) -> Option<(DeviceId, String)> {
    let ip = address.map(IpAddr::V4);
    let dns = unqualified(dns_name);
    let label = dns.split('.').next().unwrap_or_default();
    let by_ip = || {
        devices
            .iter()
            .find(|device| ip.is_some() && device.ip == ip)
    };
    let by_name = || {
        devices
            .iter()
            .find(|device| !dns.is_empty() && unqualified(&device.host) == dns)
    };
    let by_label = || {
        devices.iter().find(|device| {
            !label.is_empty() && !device.host.contains('.') && device.host.to_lowercase() == label
        })
    };
    by_ip()
        .or_else(by_name)
        .or_else(by_label)
        .map(|device| (device.id, device.name.clone()))
}

fn unqualified(name: &str) -> String {
    name.strip_suffix('.').unwrap_or(name).to_lowercase()
}

/// BD-6: a router serves a site when one of its primary routes is the site subnet exactly.
fn serves(routes: &[IpNet], subnet: IpNet) -> bool {
    routes.iter().any(|route| route.trunc() == subnet.trunc())
}

fn site_path(
    site: SiteId,
    subnet: IpNet,
    nodes: &[VpnNode],
    ping_checked: &HashSet<DeviceId>,
) -> SitePath {
    let router = nodes
        .iter()
        .position(|node| node.role == Role::Router && serves(&node.routes, subnet));
    let found = router.map(|index| &nodes[index]);
    let state = match found {
        Some(node) if node.online => RouteState::Serving,
        Some(_) => RouteState::Offline,
        None => RouteState::NoRouter,
    };
    let has_ping_check = found
        .and_then(|node| node.device.as_ref())
        .is_some_and(|(id, _)| ping_checked.contains(id));
    SitePath {
        site,
        router,
        state,
        has_ping_check,
        waiting_router: None,
    }
}
