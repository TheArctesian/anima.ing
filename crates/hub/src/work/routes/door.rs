//! The hub's side of the one interface of the proxy (Section 12.3, SF-231).
//!
//! The proxy runs as a process apart from the hub. It reaches the hub only through this Unix
//! socket, with the requests of `anima_route_proxy::wire`: the table of the names and the
//! streams that serve, a connection to a route by the identifier of its application and its
//! name, and its counts. The hub maps them to the device and the port of its own record
//! ([`super::target_of`]); no request can name a device, an address or a port, so the proxy can
//! never reach port 22 of a machine. A UDP stream goes only to an address that is not on the
//! link (OD-HOST-48): the hub sends its datagrams from a socket of its own.
//! The socket is served only when `ANIMA_ROUTES_SOCKET` is set: off by default. It has mode
//! `0660` and answers only a peer with the user ID of the proxy (`ANIMA_ROUTES_PROXY_UID`,
//! SF-329). A request line must come within [`REQUEST_WAIT`], and a stream ends
//! after [`Door::idle`] with no byte (SF-327).
//!
//! Each device has a limit of the streams of its routes at one time, below the streams of its
//! link, so the public traffic leaves room for the sessions of the operator on the same tunnel
//! (SF-230, SF-327). It counts each route of the device together. I/O edge; it writes
//! no body and no header to a log (LOG-007).

use std::{
    collections::HashMap,
    fs, io,
    net::SocketAddr,
    os::unix::fs::{FileTypeExt as _, PermissionsExt as _},
    path::Path,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};

use anima_route_proxy::{
    relay::Idle,
    streams::datagrams,
    wire::{self, Answer, Dial, Named, Reach, Request, StreamRoute, Table, Transport},
};
use async_trait::async_trait;
use sqlx::PgPool;
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt as _, BufReader, copy_bidirectional},
    net::{UdpSocket, UnixListener, UnixStream},
    time::timeout,
};
use zeroize::Zeroizing;

use super::{
    Route, Traffic, count,
    kinds::{Access, Kind},
    served, served_streams, target_of,
};
use crate::{
    clock::Clock,
    fleet::{
        credentials::Vault,
        targets::{self, AddressPolicy, on_link},
    },
    ids::{ApplicationId, DeviceId},
};

/// The longest wait for the request line of the proxy.
pub const REQUEST_WAIT: Duration = Duration::from_secs(10);
/// The default of [`Door::idle`].
pub const IDLE_DEFAULT: Duration = Duration::from_secs(300);

/// A stream to the port of an application.
pub trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}

/// How the hub reaches a fixed target: the hub's one connect function, or a double in tests.
#[async_trait]
pub trait Connector: Send + Sync {
    async fn connect(&self, address: SocketAddr) -> io::Result<Box<dyn Stream>>;
}

/// The hub's one connect function (task HM-T12), through the link for a link address.
pub struct Network(pub AddressPolicy);

#[async_trait]
impl Connector for Network {
    async fn connect(&self, address: SocketAddr) -> io::Result<Box<dyn Stream>> {
        let reached = targets::connect(&self.0, address).await?;
        Ok(Box::new(reached))
    }
}

/// The streams of the routes of each device at one time, each route together: below the streams
/// of its link (`ANIMA_ROUTES_STREAMS_PER_DEVICE`; SF-230, SF-327).
#[derive(Clone)]
pub struct DeviceStreams {
    limit: usize,
    open: Arc<Mutex<HashMap<DeviceId, usize>>>,
}

/// One stream that counts against its device until it is dropped.
pub struct StreamPermit {
    device: DeviceId,
    open: Arc<Mutex<HashMap<DeviceId, usize>>>,
}

impl DeviceStreams {
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            open: Arc::default(),
        }
    }

    /// A permit for one more stream to `device`; `None` at the limit.
    #[must_use]
    pub fn take(&self, device: DeviceId) -> Option<StreamPermit> {
        let mut open = self.open.lock().unwrap_or_else(PoisonError::into_inner);
        let streams = open.entry(device).or_default();
        if *streams >= self.limit {
            return None;
        }
        *streams += 1;
        Some(StreamPermit {
            device,
            open: Arc::clone(&self.open),
        })
    }
}

impl Drop for StreamPermit {
    fn drop(&mut self) {
        let mut open = self.open.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(streams) = open.get_mut(&self.device) {
            *streams = streams.saturating_sub(1);
        }
    }
}

/// What the interface needs.
pub struct Door {
    pub db: PgPool,
    pub vault: Vault,
    pub clock: Arc<dyn Clock>,
    pub connector: Arc<dyn Connector>,
    pub streams: DeviceStreams,
    /// The user ID of the proxy process: the one peer that the socket answers.
    pub proxy_uid: u32,
    /// The time with no byte after which a stream to an application ends.
    pub idle: Duration,
}

/// Makes the socket of the interface at `path`, with mode `0660`: the proxy reaches it through
/// the group of the socket. A socket an earlier hub left at `path` is replaced; any other file
/// there is refused.
///
/// # Errors
/// `path` holds a file that is not a socket, or the socket cannot be made.
pub fn bind(path: &Path) -> io::Result<UnixListener> {
    match fs::symlink_metadata(path) {
        Ok(left) if left.file_type().is_socket() => fs::remove_file(path)?,
        Ok(_) => return Err(io::ErrorKind::AlreadyExists.into()),
        Err(_) => {}
    }
    let listener = UnixListener::bind(path)?;
    if let Err(error) = fs::set_permissions(path, fs::Permissions::from_mode(0o660)) {
        let _ = fs::remove_file(path);
        return Err(error);
    }
    Ok(listener)
}

/// Whether the peer of `socket` runs as `proxy_uid`.
fn from_proxy(socket: &UnixStream, proxy_uid: u32) -> bool {
    socket.peer_cred().is_ok_and(|peer| peer.uid() == proxy_uid)
}

/// Serves the interface on `listener` until the task is stopped. A connection from a process
/// that is not the proxy is closed with no answer.
pub async fn serve(listener: UnixListener, door: Arc<Door>) {
    loop {
        let Ok((socket, _)) = listener.accept().await else {
            continue;
        };
        if !from_proxy(&socket, door.proxy_uid) {
            tracing::warn!("a process that is not the route proxy opened the socket of the proxy");
            continue;
        }
        let door = Arc::clone(&door);
        tokio::spawn(async move {
            if let Err(error) = answer(&door, socket).await {
                tracing::warn!(error = %error, "a request of the route proxy failed");
            }
        });
    }
}

/// Why one request was not answered. Holds no key and no byte of a stream.
#[derive(Debug, thiserror::Error)]
pub enum DoorError {
    #[error(transparent)]
    Wire(#[from] wire::WireError),
    #[error("the routes cannot be read")]
    Routes(#[from] super::Error),
    #[error("the stream ended with an error")]
    Stream(#[from] io::Error),
}

/// Answers the one request on `socket`.
///
/// # Errors
/// [`DoorError`] for a request that is not one of the interface, or a failure of the hub.
pub async fn answer<S: AsyncRead + AsyncWrite + Unpin + Send>(
    door: &Door,
    socket: S,
) -> Result<(), DoorError> {
    let mut socket = BufReader::new(socket);
    let Ok(read) = timeout(
        REQUEST_WAIT,
        wire::read_line(&mut socket, wire::REQUEST_MAX),
    )
    .await
    else {
        return Ok(());
    };
    let Some(line) = read? else {
        return Ok(());
    };
    let now = door.clock.now();
    match wire::parse_request(&line)? {
        Request::Table => {
            let table = Table {
                names: served(&door.db, &door.vault, now)
                    .await?
                    .into_iter()
                    .map(|route| Named {
                        name: route.name.to_string(),
                        application: route.application.into(),
                        chain_pem: route.chain_pem,
                        key_pem: Zeroizing::new(
                            String::from_utf8_lossy(route.key_pem.expose()).into_owned(),
                        ),
                        reach: reach(route.access),
                        redirect_to: route.redirect_to,
                    })
                    .collect(),
                challenges: Vec::new(),
                streams: served_streams(&door.db, now)
                    .await?
                    .iter()
                    .filter_map(stream_route)
                    .collect(),
            };
            wire::write_json(&mut socket, &table).await?;
        }
        Request::Connect(dial) => connect(door, &mut socket, dial).await?,
        Request::Count { name, counts } => {
            let traffic = Traffic {
                requests: counts.requests,
                bytes_in: counts.bytes_in,
                bytes_out: counts.bytes_out,
                refused: counts.refused,
            };
            count(&door.db, &name, traffic, now).await?;
            say(&mut socket, Answer::Ok).await?;
        }
    }
    Ok(())
}

fn reach(access: Access) -> Reach {
    match access {
        Access::Public => Reach::Public,
        Access::Private => Reach::Private,
    }
}

/// The wire form of a serving stream.
fn stream_route(route: &Route) -> Option<StreamRoute> {
    Some(StreamRoute {
        name: route.name.to_string(),
        application: route.application.into(),
        transport: match route.kind {
            Kind::TcpStream => Transport::Tcp,
            Kind::UdpStream => Transport::Udp,
            Kind::Web | Kind::Redirect => return None,
        },
        listen_port: route.listen_port?,
        reach: reach(route.access),
    })
}

/// Whether a route of `kind` carries `transport`: TCP for a web route and a TCP stream, UDP for
/// a UDP stream. A redirect carries nothing to the machine.
fn carries(kind: Kind, transport: Transport) -> bool {
    matches!(
        (kind, transport),
        (Kind::Web | Kind::TcpStream, Transport::Tcp) | (Kind::UdpStream, Transport::Udp)
    )
}

/// Connects the socket to the fixed target of the route of `dial`, or answers why not.
async fn connect<S: AsyncRead + AsyncWrite + Unpin + Send>(
    door: &Door,
    socket: &mut BufReader<S>,
    dial: Dial,
) -> Result<(), DoorError> {
    let application = ApplicationId::from(dial.application);
    let found = target_of(&door.db, application, &dial.name, door.clock.now()).await;
    let (device, address) = match found {
        Ok((device, address, kind)) if carries(kind, dial.transport) => (device, address),
        Ok(_) | Err(super::Error::NotFound) => return say(socket, Answer::Refused).await,
        Err(error) => return Err(error.into()),
    };
    let Some(_permit) = door.streams.take(device) else {
        return say(socket, Answer::Refused).await;
    };
    if dial.transport == Transport::Udp {
        return datagrams_to(door, socket, address).await;
    }
    let Ok(stream) = door.connector.connect(address).await else {
        return say(socket, Answer::Unavailable).await;
    };
    say(socket, Answer::Ok).await?;
    let mut stream = Idle::new(stream, door.idle);
    copy_bidirectional(socket, &mut stream).await?;
    Ok(())
}

/// Carries the datagrams of a UDP stream between the socket of the proxy and `address`, from a
/// UDP socket of the hub. Never to an address on the link (OD-HOST-48).
async fn datagrams_to<S: AsyncRead + AsyncWrite + Unpin + Send>(
    door: &Door,
    socket: &mut BufReader<S>,
    address: SocketAddr,
) -> Result<(), DoorError> {
    if on_link(address.ip().to_canonical()) {
        return say(socket, Answer::Refused).await;
    }
    let local: SocketAddr = if address.is_ipv4() {
        ([0, 0, 0, 0], 0).into()
    } else {
        ([0_u16; 8], 0).into()
    };
    let udp = UdpSocket::bind(local).await?;
    if udp.connect(address).await.is_err() {
        return say(socket, Answer::Unavailable).await;
    }
    say(socket, Answer::Ok).await?;
    datagrams::pump(socket, &udp, door.idle).await?;
    Ok(())
}

async fn say<S: AsyncWrite + Unpin>(socket: &mut S, answer: Answer) -> Result<(), DoorError> {
    socket.write_all(answer.word().as_bytes()).await?;
    socket.write_all(b"\n").await?;
    socket.flush().await?;
    Ok(())
}
