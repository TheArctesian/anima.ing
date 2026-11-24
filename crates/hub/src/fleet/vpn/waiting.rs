//! What the broker sees that the hub's own status does not show yet (BD-16). Pure.

use std::net::Ipv4Addr;

use ipnet::IpNet;

use super::{
    broker::{BrokerNode, BrokerView, ipv4_of},
    model::{VpnNode, VpnPath},
    peers::shown,
    picture::{Inputs, device_by, key_state, role},
};

/// As `broker/protocol.ts` accepts.
const SITE_NAME_MAX: usize = 32;

pub(super) fn routes(view: &BrokerView, address: Ipv4Addr) -> Vec<IpNet> {
    view.nodes
        .iter()
        .find(|node| ipv4_of(node) == Some(address))
        .map_or_default(unapproved)
}

/// The broker takes a policy site name, never an Anima site name. It also goes into a command
/// line the operator copies, so only `[a-z0-9-]{1,32}` passes.
pub(super) fn policy_site(view: &BrokerView, subnet: IpNet) -> Option<&str> {
    view.sites
        .iter()
        .find(|(_, policy)| policy.trunc() == subnet.trunc())
        .map(|(name, _)| name.as_str())
        .filter(|name| is_site_name(name))
}

pub(super) fn router(view: &BrokerView, subnet: IpNet, inputs: &Inputs<'_>) -> Option<VpnNode> {
    let site_tag = format!("tag:site-{}", policy_site(view, subnet)?);
    view.nodes
        .iter()
        .filter(|node| has_tag(node, "tag:router") && has_tag(node, &site_tag))
        .filter(|node| unapproved(node).contains(&subnet.trunc()))
        .filter_map(|node| waiting_node(node, inputs))
        .min_by(|a, b| a.name.cmp(&b.name))
}

pub(super) fn is_site_name(name: &str) -> bool {
    (1..=SITE_NAME_MAX).contains(&name.len())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn has_tag(node: &BrokerNode, tag: &str) -> bool {
    node.tags.iter().any(|own| own == tag)
}

fn unapproved(node: &BrokerNode) -> Vec<IpNet> {
    node.available
        .iter()
        .map(IpNet::trunc)
        .filter(|route| !node.approved.iter().any(|done| done.trunc() == *route))
        .collect()
}

/// The name and tags come from Headscale and are cleaned (VPN-008).
fn waiting_node(node: &BrokerNode, inputs: &Inputs<'_>) -> Option<VpnNode> {
    let address = ipv4_of(node)?;
    Some(VpnNode {
        // No node key: the broker drops every key (BD-9), so no warning mark is made for it.
        key: String::new(),
        name: shown(&node.name, 63),
        address,
        os: String::new(),
        role: role(&node.tags),
        tags: node.tags.iter().map(|tag| shown(tag, 63)).collect(),
        routes: node.approved.iter().map(IpNet::trunc).collect(),
        waiting_routes: unapproved(node),
        online: node.online,
        last_seen: None,
        key_expiry: node.expiry,
        key_state: key_state(&node.tags, node.expiry, inputs.now),
        device: device_by(Some(address), &node.name, inputs.devices),
        in_anima: false,
        path: VpnPath::Unknown,
        relay_region: None,
        unknown_reads: 0,
    })
}
