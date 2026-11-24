//! The loopback verdict over a server's socket table (Section 9, RTC-006, SEC-C-001); the daemon never connects to find a socket.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    Tcp,
    Udp,
}

/// One listening TCP socket or one bound UDP socket.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Socket {
    pub protocol: Protocol,
    pub address: IpAddr,
    pub port: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Loopback,
    Exposed(Socket),
}

#[must_use]
pub fn is_loopback(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => v4 == Ipv4Addr::LOCALHOST,
        IpAddr::V6(v6) => v6 == Ipv6Addr::LOCALHOST || v6 == Ipv4Addr::LOCALHOST.to_ipv6_mapped(),
    }
}

#[must_use]
pub fn verdict(sockets: &[Socket]) -> Verdict {
    sockets
        .iter()
        .find(|socket| !is_loopback(socket.address))
        .map_or(Verdict::Loopback, |socket| Verdict::Exposed(*socket))
}

/// A loopback TCP listener on `port` means the server is ready and the listener belongs to its tree.
#[must_use]
pub fn listens(sockets: &[Socket], port: u16) -> bool {
    sockets.iter().any(|socket| {
        socket.protocol == Protocol::Tcp && socket.port == port && is_loopback(socket.address)
    })
}

fn hex_v4(text: &str) -> Option<IpAddr> {
    let raw = u32::from_str_radix(text, 16).ok()?;
    Some(IpAddr::V4(Ipv4Addr::from(raw.swap_bytes())))
}

fn hex_v6(text: &str) -> Option<IpAddr> {
    if text.len() != 32 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for (word, chunk) in bytes.chunks_mut(4).enumerate() {
        let raw = u32::from_str_radix(text.get(word * 8..word * 8 + 8)?, 16).ok()?;
        chunk.copy_from_slice(&raw.swap_bytes().to_be_bytes());
    }
    Some(IpAddr::V6(Ipv6Addr::from(bytes)))
}

/// Reads one `/proc/<pid>/net` table: each listening TCP socket (state `0A`) and bound UDP socket, with its inode.
#[must_use]
pub fn read_proc_net(text: &str, protocol: Protocol) -> Vec<(Socket, u64)> {
    text.lines()
        .skip(1)
        .filter_map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let (local, state, inode) = (fields.get(1)?, fields.get(3)?, fields.get(9)?);
            let (host, port) = local.split_once(':')?;
            let address = if host.len() == 8 {
                hex_v4(host)?
            } else {
                hex_v6(host)?
            };
            let port = u16::from_str_radix(port, 16).ok()?;
            let counts = match protocol {
                Protocol::Tcp => *state == "0A",
                Protocol::Udp => port != 0,
            };
            counts.then(|| {
                Some((
                    Socket {
                        protocol,
                        address,
                        port,
                    },
                    inode.parse().ok()?,
                ))
            })?
        })
        .collect()
}

fn lsof_name(name: &str, protocol: Protocol) -> Option<Socket> {
    let local = name.split("->").next()?;
    let (host, port) = local.rsplit_once(':')?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    let address = if host == "*" {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    } else {
        host.parse().ok()?
    };
    Some(Socket {
        protocol,
        address,
        port: port.parse().ok()?,
    })
}

#[derive(Default)]
struct LsofFile<'a> {
    protocol: Option<Protocol>,
    name: Option<&'a str>,
    listen: bool,
}

impl LsofFile<'_> {
    fn socket(&self) -> Option<Socket> {
        let protocol = self.protocol?;
        if protocol == Protocol::Tcp && !self.listen {
            return None;
        }
        // A UDP socket with no port (`*:*`, or an address and `*`) is not bound.
        let local = self.name.and_then(|name| name.split("->").next());
        if protocol == Protocol::Udp && local.is_some_and(|local| local.ends_with(":*")) {
            return None;
        }
        let unreadable = Socket {
            protocol,
            address: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            port: 0,
        };
        Some(
            self.name
                .and_then(|name| lsof_name(name, protocol))
                .unwrap_or(unreadable),
        )
    }
}

/// Reads `lsof -F PnT` output (macOS); a name that does not read counts as exposed.
#[must_use]
pub fn read_lsof(text: &str) -> Vec<Socket> {
    let mut files = vec![LsofFile::default()];
    for line in text.lines() {
        let (tag, rest) = line.split_at(line.len().min(1));
        if matches!(tag, "f" | "p") {
            files.push(LsofFile::default());
            continue;
        }
        let Some(file) = files.last_mut() else {
            continue;
        };
        match tag {
            "P" => {
                file.protocol = match rest {
                    "TCP" => Some(Protocol::Tcp),
                    "UDP" => Some(Protocol::Udp),
                    _ => None,
                };
            }
            "n" => file.name = Some(rest),
            "T" if rest == "ST=LISTEN" => file.listen = true,
            _ => {}
        }
    }
    files.iter().filter_map(LsofFile::socket).collect()
}
