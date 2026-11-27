//! The hub's one connect function: by the network, or for a link address through the link's dial socket (HMD-048).
//! The computer decides which of its ports the hub may reach (HMD-049).
//! The dial socket must be served by the hub's own user; SSH pins the host key before a credential leaves (SEC-007).

use std::{
    io,
    net::{IpAddr, Ipv6Addr, SocketAddr},
    path::Path,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _, ReadBuf},
    net::TcpStream,
};

#[cfg(windows)]
use anima_windows::socket::Stream as UnixStream;
#[cfg(unix)]
use tokio::net::UnixStream;

use super::AddressPolicy;
use crate::config::LINK_DEVICES;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

/// The link's own wait for the computer is 10 s.
const LINK_ANSWER: Duration = Duration::from_secs(15);
const WORD_MAX: usize = 32;

#[derive(Debug)]
pub enum Reached {
    Tcp(TcpStream),
    Link(UnixStream),
}

impl Reached {
    #[must_use]
    pub fn network_peer(&self) -> Option<IpAddr> {
        match self {
            Self::Tcp(stream) => stream.peer_addr().ok().map(|peer| peer.ip()),
            Self::Link(_) => None,
        }
    }
}

/// The caller has checked `address` against `policy`.
///
/// # Errors
/// As a TCP connect; through the link, the link's refusal as an [`io::ErrorKind`].
pub async fn connect(policy: &AddressPolicy, address: SocketAddr) -> io::Result<Reached> {
    if let (Some(socket), IpAddr::V6(device)) = (policy.link_dial(), address.ip())
        && LINK_DEVICES.contains(&device)
    {
        let dialled = dial(socket, device, address.port());
        return tokio::time::timeout(LINK_ANSWER, dialled)
            .await
            .map_err(|_| io::ErrorKind::TimedOut)?
            .map(Reached::Link);
    }
    TcpStream::connect(address).await.map(Reached::Tcp)
}

/// The hub reaches a link address with TCP only, so an ICMP or UDP check cannot run on it (HMD-048).
#[must_use]
pub fn on_link(ip: IpAddr) -> bool {
    matches!(ip, IpAddr::V6(device) if LINK_DEVICES.contains(&device))
}

async fn dial(socket: &Path, device: Ipv6Addr, port: u16) -> io::Result<UnixStream> {
    dial_line(socket, format!("dial {device} {port}\n")).await
}

/// Connect only a caller-authorized workspace binding through the hub's private link.
/// No TCP fallback is permitted for a workspace namespace.
///
/// # Errors
/// Returns an error for a denied or expired binding, an unavailable private link,
/// an invalid request encoding, or a failed or timed-out link connection.
pub async fn preview(
    policy: &AddressPolicy,
    device: Ipv6Addr,
    request: &agent_threads::managed::preview::Open,
) -> io::Result<Reached> {
    if !LINK_DEVICES.contains(&device)
        || !policy.permits(IpAddr::V6(device))
        || !request.valid(chrono::Utc::now())
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let socket = policy.link_dial().ok_or(io::ErrorKind::NotConnected)?;
    let bytes = serde_json::to_vec(request).map_err(|_| io::ErrorKind::InvalidInput)?;
    if bytes.len() > 640 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    tokio::time::timeout(
        LINK_ANSWER,
        dial_line(socket, format!("preview {device} {encoded}\n")),
    )
    .await
    .map_err(|_| io::ErrorKind::TimedOut)?
    .map(Reached::Link)
}

/// Connect only a caller-authorized provider sign-in through the hub's private link.
/// No TCP fallback is permitted for a provider sign-in.
///
/// # Errors
/// Returns an error for a denied or expired sign-in, an unavailable private link,
/// an invalid request encoding, or a failed or timed-out link connection.
pub async fn provider_login(
    policy: &AddressPolicy,
    device: Ipv6Addr,
    request: &agent_threads::managed::provider_login::Login,
) -> io::Result<Reached> {
    if !LINK_DEVICES.contains(&device)
        || !policy.permits(IpAddr::V6(device))
        || !request.valid(chrono::Utc::now())
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let socket = policy.link_dial().ok_or(io::ErrorKind::NotConnected)?;
    let bytes = serde_json::to_vec(request).map_err(|_| io::ErrorKind::InvalidInput)?;
    if bytes.len() > 640 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let encoded = URL_SAFE_NO_PAD.encode(bytes);
    tokio::time::timeout(
        LINK_ANSWER,
        dial_line(socket, format!("provider {device} {encoded}\n")),
    )
    .await
    .map_err(|_| io::ErrorKind::TimedOut)?
    .map(Reached::Link)
}

async fn dial_line(socket: &Path, line: String) -> io::Result<UnixStream> {
    let mut stream = UnixStream::connect(socket).await?;
    #[cfg(unix)]
    if stream.peer_cred()?.uid() != rustix::process::geteuid().as_raw() {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    stream.write_all(line.as_bytes()).await?;
    // One byte at a time: the bytes after the line are the device's.
    let mut word = Vec::with_capacity(WORD_MAX);
    loop {
        match stream.read_u8().await? {
            b'\n' => break,
            _ if word.len() == WORD_MAX => return Err(io::ErrorKind::InvalidData.into()),
            byte => word.push(byte),
        }
    }
    let refusal = match word.as_slice() {
        b"ok" => return Ok(stream),
        b"refused" => io::ErrorKind::PermissionDenied,
        b"closed" => io::ErrorKind::ConnectionRefused,
        b"not_connected" => io::ErrorKind::NotConnected,
        b"timeout" => io::ErrorKind::TimedOut,
        _ => io::ErrorKind::InvalidData,
    };
    Err(refusal.into())
}

impl AsyncRead for Reached {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Tcp(stream) => Pin::new(stream).poll_read(context, buffer),
            Self::Link(stream) => Pin::new(stream).poll_read(context, buffer),
        }
    }
}

impl AsyncWrite for Reached {
    fn poll_write(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Tcp(stream) => Pin::new(stream).poll_write(context, bytes),
            Self::Link(stream) => Pin::new(stream).poll_write(context, bytes),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Tcp(stream) => Pin::new(stream).poll_flush(context),
            Self::Link(stream) => Pin::new(stream).poll_flush(context),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Tcp(stream) => Pin::new(stream).poll_shutdown(context),
            Self::Link(stream) => Pin::new(stream).poll_shutdown(context),
        }
    }
}
