//! The PROXY protocol header (version 1) that the door of the cloud service puts before each connection (Section 12.2).
//! It is read only in door mode and only from a door's address, so a client cannot claim an address (SF-229, SF-327).

use std::net::IpAddr;

/// The longest version 1 header, with its line end (the specification gives 107).
pub const HEADER_MAX: usize = 107;

/// The client address of a version 1 header line, without its `\r\n`.
#[must_use]
pub fn client(line: &str) -> Option<IpAddr> {
    let mut parts = line.split(' ');
    let (Some("PROXY"), Some(family), Some(source)) = (parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let address: IpAddr = source.parse().ok()?;
    match (family, address) {
        ("TCP4", IpAddr::V4(_)) | ("TCP6", IpAddr::V6(_)) => Some(address),
        _ => None,
    }
}

#[must_use]
pub fn from_door(peer: IpAddr, doors: &[IpAddr]) -> bool {
    let peer = peer.to_canonical();
    doors.iter().any(|door| door.to_canonical() == peer)
}
