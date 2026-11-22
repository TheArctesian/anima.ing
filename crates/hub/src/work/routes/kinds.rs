//! The kinds and the access of a route (HOST-050, task H40; Section 16 of
//! `docs/proposals/hosted-applications-stage-2.md`): what each kind needs, the port of the hub
//! that a stream listens on, and the protocols that may have a public stream. Pure.
//!
//! The operator's decisions apply: no stream on a hosted hub until the door has an
//! address for each hub (OD-HOST-47); UDP only to a machine that the hub reaches on its own
//! network, never through the link (OD-HOST-48, checked in [`super::add`] when the address is
//! fixed); the access is public or private (OD-HOST-49).

use std::ops::RangeInclusive;

/// What a route carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// HTTPS by name to the port of the application (Section 12).
    Web,
    /// A proven name that answers with a fixed `https://` address.
    Redirect,
    /// A port of the hub to the port of the application, over TCP.
    TcpStream,
    /// A port of the hub to the port of the application, over UDP.
    UdpStream,
}

impl Kind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Redirect => "redirect",
            Self::TcpStream => "tcp_stream",
            Self::UdpStream => "udp_stream",
        }
    }

    #[must_use]
    pub fn from_stored(text: &str) -> Option<Self> {
        [Self::Web, Self::Redirect, Self::TcpStream, Self::UdpStream]
            .into_iter()
            .find(|kind| kind.as_str() == text)
    }

    /// Whether the route is a stream: a port of the hub, no TLS of the hub and no name in it.
    #[must_use]
    pub const fn is_stream(self) -> bool {
        matches!(self, Self::TcpStream | Self::UdpStream)
    }
}

/// Who may reach a route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// The internet.
    Public,
    /// Only the nodes of the network of Anima, on the address of the hub on that network
    /// (Section 16.7, SF-245). Each machine and container of that network is a node.
    Private,
}

impl Access {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }

    #[must_use]
    pub fn from_stored(text: &str) -> Option<Self> {
        [Self::Public, Self::Private]
            .into_iter()
            .find(|access| access.as_str() == text)
    }
}

/// A protocol that may have a public stream: it has its own sign-in and its own TLS, so a stream
/// with no look inside does not open a service that trusts its network (SF-248). The
/// usual port is a hint for the person; the hub never opens a port by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Protocol {
    pub name: &'static str,
    pub usual_port: u16,
    pub udp: bool,
}

/// The fixed list of the protocols of a public stream. No database, MQTT, Redis, syslog or shell.
pub const PUBLIC_PROTOCOLS: [Protocol; 7] = [
    Protocol {
        name: "imaps",
        usual_port: 993,
        udp: false,
    },
    Protocol {
        name: "pop3s",
        usual_port: 995,
        udp: false,
    },
    Protocol {
        name: "smtps",
        usual_port: 465,
        udp: false,
    },
    Protocol {
        name: "ldaps",
        usual_port: 636,
        udp: false,
    },
    Protocol {
        name: "xmpps",
        usual_port: 5223,
        udp: false,
    },
    Protocol {
        name: "openvpn",
        usual_port: 1194,
        udp: true,
    },
    Protocol {
        name: "wireguard",
        usual_port: 51820,
        udp: true,
    },
];

/// The ports of the hub that a stream never takes, also inside the range of the operator, besides
/// the ports of Anima of the catalogue (`catalogue::ANIMA_PORTS`: the gateway, the local hub and the
/// link with its STUN): PostgreSQL and `WireGuard` (Section 16.4).
const RESERVED_LISTEN_PORTS: [u16; 2] = [5432, 51820];
/// The lowest port of the hub for a stream: the ports below it are the system's.
const LOWEST_LISTEN_PORT: u16 = 1024;

/// Why the kind, the access or the port of the hub of a route is refused. Holds no address.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KindRefusal {
    #[error("a stream needs a port of the hub (listenPort)")]
    NoListenPort,
    #[error("only a stream has a port of the hub")]
    ListenPortNotForKind,
    #[error("this hub has no range of ports for streams (ANIMA_ROUTES_STREAM_PORTS)")]
    NoStreamPorts,
    #[error("a port of the hub below 1024 is refused")]
    ListenPortBelow1024,
    #[error("the port of the hub is outside the range of ports for streams")]
    ListenPortOutsideRange,
    #[error("the port of the hub belongs to Anima, the link or the database")]
    ListenPortReserved,
    #[error("a hosted hub has no stream until the door has an address for each hub (OD-HOST-47)")]
    StreamOnHostedHub,
    #[error(
        "UDP goes only to a machine that the hub reaches on its own network, never through the link (OD-HOST-48)"
    )]
    UdpThroughLink,
    #[error("a public stream needs a protocol with its own sign-in and TLS: {0}")]
    PublicStreamProtocol(String),
    #[error("only a stream has a protocol")]
    ProtocolNotForKind,
    #[error("a redirect needs an https:// address (redirectTo) and no port")]
    RedirectShape,
    #[error("only a redirect has an address to send to")]
    RedirectToNotForKind,
    #[error("the address of a redirect is not a plain https:// address")]
    RedirectTo,
    #[error("this kind needs the port of the application")]
    NoPort,
}

/// What the hub permits for streams.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StreamRules {
    /// The hub is hosted by the cloud service (OD-HOST-47).
    pub hosted: bool,
    /// The ports of the hub that the operator gave to streams; `None`: no stream.
    pub ports: Option<RangeInclusive<u16>>,
    /// The ports that the hub itself listens on.
    pub hub_ports: Vec<u16>,
}

/// What a person asks for, besides the name and the target.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    pub kind: Kind,
    pub access: Access,
    pub port: Option<i64>,
    pub listen_port: Option<i64>,
    pub protocol: Option<String>,
    pub redirect_to: Option<String>,
}

/// A shape that passed [`check`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    pub listen_port: Option<u16>,
    pub protocol: Option<&'static str>,
    pub redirect_to: Option<String>,
}

/// Checks a shape against the rules of its kind.
///
/// # Errors
/// The [`KindRefusal`] of the first rule that refuses.
pub fn check(shape: &Shape, rules: &StreamRules) -> Result<Checked, KindRefusal> {
    if shape.kind == Kind::Redirect {
        if shape.port.is_some() || shape.listen_port.is_some() || shape.protocol.is_some() {
            return Err(KindRefusal::RedirectShape);
        }
        let to = shape
            .redirect_to
            .as_deref()
            .ok_or(KindRefusal::RedirectShape)?;
        return Ok(Checked {
            listen_port: None,
            protocol: None,
            redirect_to: Some(redirect_to(to)?),
        });
    }
    if shape.redirect_to.is_some() {
        return Err(KindRefusal::RedirectToNotForKind);
    }
    if shape.port.is_none() {
        return Err(KindRefusal::NoPort);
    }
    if !shape.kind.is_stream() {
        if shape.listen_port.is_some() {
            return Err(KindRefusal::ListenPortNotForKind);
        }
        if shape.protocol.is_some() {
            return Err(KindRefusal::ProtocolNotForKind);
        }
        return Ok(Checked {
            listen_port: None,
            protocol: None,
            redirect_to: None,
        });
    }
    if rules.hosted {
        return Err(KindRefusal::StreamOnHostedHub);
    }
    let listen_port = listen_port(shape.listen_port.ok_or(KindRefusal::NoListenPort)?, rules)?;
    let protocol = protocol(shape)?;
    Ok(Checked {
        listen_port: Some(listen_port),
        protocol,
        redirect_to: None,
    })
}

/// The port of the hub of a stream.
fn listen_port(port: i64, rules: &StreamRules) -> Result<u16, KindRefusal> {
    let range = rules.ports.as_ref().ok_or(KindRefusal::NoStreamPorts)?;
    let port = u16::try_from(port).map_err(|_| KindRefusal::ListenPortOutsideRange)?;
    if port < LOWEST_LISTEN_PORT {
        return Err(KindRefusal::ListenPortBelow1024);
    }
    if catalogue::ANIMA_PORTS.contains(&port)
        || RESERVED_LISTEN_PORTS.contains(&port)
        || rules.hub_ports.contains(&port)
    {
        return Err(KindRefusal::ListenPortReserved);
    }
    if !range.contains(&port) {
        return Err(KindRefusal::ListenPortOutsideRange);
    }
    Ok(port)
}

/// The protocol of a stream: one of [`PUBLIC_PROTOCOLS`] for its transport when it is public;
/// a private stream may name one or none.
fn protocol(shape: &Shape) -> Result<Option<&'static str>, KindRefusal> {
    let udp = shape.kind == Kind::UdpStream;
    let found = shape.protocol.as_deref().map(|name| {
        PUBLIC_PROTOCOLS
            .iter()
            .find(|protocol| protocol.name == name.to_ascii_lowercase() && protocol.udp == udp)
    });
    match (shape.access, found) {
        (_, Some(Some(protocol))) => Ok(Some(protocol.name)),
        (Access::Private, None) => Ok(None),
        _ => Err(KindRefusal::PublicStreamProtocol(protocols_of(udp))),
    }
}

/// The protocols of one transport with their usual ports, for a refusal.
fn protocols_of(udp: bool) -> String {
    PUBLIC_PROTOCOLS
        .iter()
        .filter(|protocol| protocol.udp == udp)
        .map(|protocol| format!("{} (usual port {})", protocol.name, protocol.usual_port))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The address of a redirect: `https://`, a host of letters, digits, dots and hyphens with an
/// optional port, and a path of visible characters with no space, no `\` and no `@` before the
/// path, at most 2048 bytes. A redirect always goes to this one address: the proxy adds nothing
/// of the request to it, so the name cannot be an open redirect (SF-248).
///
/// # Errors
/// [`KindRefusal::RedirectTo`].
pub fn redirect_to(to: &str) -> Result<String, KindRefusal> {
    let rest = to.strip_prefix("https://").ok_or(KindRefusal::RedirectTo)?;
    let visible = to.len() <= 2048 && to.bytes().all(|byte| byte.is_ascii_graphic());
    let (authority, path) = rest.split_at(rest.find(['/', '?', '#']).unwrap_or(rest.len()));
    let (host, port) = authority
        .rsplit_once(':')
        .map_or((authority, None), |(host, port)| (host, Some(port)));
    let host_ok = !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        });
    let port_ok = port.is_none_or(|port| port.parse::<u16>().is_ok_and(|port| port > 0));
    if !visible || !host_ok || !port_ok || path.contains('\\') {
        return Err(KindRefusal::RedirectTo);
    }
    Ok(format!("https://{}{path}", authority.to_ascii_lowercase()))
}
