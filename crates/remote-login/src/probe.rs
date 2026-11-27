//! Credential-free observations of the fixed appliance listeners.
use super::{vnc, x224};
use std::{io, net::Ipv4Addr, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWrite},
    net::TcpStream,
    time::{Instant, timeout_at},
};

#[derive(Clone, Copy)]
pub enum LocalProtocol {
    Ssh,
    Rdp,
    Vnc,
}

impl LocalProtocol {
    #[must_use]
    pub const fn port(self) -> u16 {
        match self {
            Self::Ssh => 22,
            Self::Rdp => 3389,
            Self::Vnc => 5900,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observation {
    Unavailable,
    NoReply,
    Unexpected,
    Responding,
}

/// Checks protocol negotiation on IPv4 loopback. Successful login, trust
/// and reachability over the private link require a separate session.
pub async fn observe(protocol: LocalProtocol) -> Observation {
    let deadline = Instant::now() + Duration::from_secs(2);
    let Ok(Ok(mut stream)) = timeout_at(
        deadline,
        TcpStream::connect((Ipv4Addr::LOCALHOST, protocol.port())),
    )
    .await
    else {
        return Observation::Unavailable;
    };
    match timeout_at(deadline, protocol_answer(protocol, &mut stream)).await {
        Ok(true) => Observation::Responding,
        Ok(false) => Observation::Unexpected,
        Err(_) => Observation::NoReply,
    }
}

async fn protocol_answer<S: AsyncRead + AsyncWrite + Unpin>(
    protocol: LocalProtocol,
    stream: &mut S,
) -> bool {
    match protocol {
        LocalProtocol::Ssh => ssh(stream).await.unwrap_or(false),
        LocalProtocol::Rdp => matches!(
            x224::negotiate(stream, x224::TLS_PROTOCOLS).await,
            Ok(x224::Negotiated::Tls(
                x224::PROTOCOL_SSL | x224::PROTOCOL_HYBRID | x224::PROTOCOL_HYBRID_EX
            ))
        ),
        LocalProtocol::Vnc => vnc::anima_os_authenticated(stream).await.unwrap_or(false),
    }
}

async fn ssh(stream: &mut (impl AsyncRead + Unpin)) -> io::Result<bool> {
    let mut prefix = [0; 8];
    stream.read_exact(&mut prefix).await?;
    Ok(&prefix == b"SSH-2.0-" || &prefix == b"SSH-1.99")
}

pub(super) async fn ready() -> bool {
    observe(LocalProtocol::Ssh).await == Observation::Responding
}

#[cfg(test)]
mod tests;
