//! Proof of the one egress path (P3-003, D-17, N-2).

mod http;
mod replies;
pub(in crate::fleet::notifications) mod servers;
mod smtp;

use ipnet::IpNet;

use super::Egress;
use crate::fleet::targets::AddressPolicy;

pub(in crate::fleet::notifications) fn local() -> Egress {
    Egress::new(AddressPolicy::default().allowing_loopback())
}

fn loopback_denied() -> Egress {
    let deny: Vec<IpNet> = vec!["127.0.0.0/8".parse().unwrap(), "::1/128".parse().unwrap()];
    Egress::new(AddressPolicy::new(deny).allowing_loopback())
}
