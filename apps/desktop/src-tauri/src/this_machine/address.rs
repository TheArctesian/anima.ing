use std::{
    net::{Ipv4Addr, SocketAddr, TcpStream},
    time::Duration,
};

/// 100.64.0.0/10 (NET-001); other overlays share it, so this never says which network an address is on (ADR-005).
pub(super) fn is_vpn_address(ip: Ipv4Addr) -> bool {
    let [first, second, ..] = ip.octets();
    first == 100 && second & 0b1100_0000 == 0b0100_0000
}

pub(super) fn accepts_connection(address: SocketAddr, patience: Duration) -> bool {
    TcpStream::connect_timeout(&address, patience).is_ok()
}

pub(super) fn ssh_answers(vpn_address: Option<Ipv4Addr>, port: u16, patience: Duration) -> bool {
    vpn_address.is_some_and(|ip| accepts_connection((ip, port).into(), patience))
}
