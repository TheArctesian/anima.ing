//! The tunnel socket (K4 Sections 8 and 9, CMD-003).
//! The ticket is the only secret (API-009): it goes only into the query of the request URL,
//! which is built in a wiped buffer, and no error holds it (CMD-014, LOG-007).
//! The hub's frames are untrusted: the caller parses every text message against K4's closed set.

use std::fmt;
use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use reqwest::Url;
use reqwest::header::HeaderValue;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
use tokio_tungstenite::tungstenite::http::header::SEC_WEBSOCKET_PROTOCOL;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, WebSocketConfig};
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use zeroize::Zeroizing;

use super::wire::{self, Wire};
use super::{Code, Hub, HubError};

/// One send may wait this long; then the tunnel counts as stalled (K4 Section 8.5).
pub const SEND_WAIT: Duration = Duration::from_secs(15);

/// How long a close waits for the hub's answer (K4 Section 8.6 waits as long).
const CLOSE_WAIT: Duration = Duration::from_secs(2);

/// The longest ticket token taken; K4's are 43 characters.
const MAX_TOKEN: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tunnel {
    Terminal,
    Files,
}

impl Tunnel {
    const fn path(self) -> &'static str {
        match self {
            Self::Terminal => "/terminal",
            Self::Files => "/files",
        }
    }

    const fn subprotocol(self) -> &'static str {
        match self {
            Self::Terminal => "anima-terminal",
            Self::Files => "anima-files",
        }
    }

    /// SSH packets are at most 32 KiB; a listing of 10 000 entries with long names fits in 16 MiB.
    const fn max_message(self) -> usize {
        match self {
            Self::Terminal => 1024 * 1024,
            Self::Files => 16 * 1024 * 1024,
        }
    }
}

/// A single-use, 30-second ticket (K4 Section 1). No `Display`; `Debug` hides the token.
pub struct Ticket {
    token: Zeroizing<String>,
    tunnel: Tunnel,
}

impl Ticket {
    /// The token must be base64url, so it goes into a query as it is.
    ///
    /// # Errors
    /// [`TunnelError::BadTicket`] for a token or a path that does not fit.
    pub fn new(token: String, tunnel_path: &str, tunnel: Tunnel) -> Result<Self, TunnelError> {
        let token = Zeroizing::new(token);
        let plain = token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_');
        let fits = plain && !token.is_empty() && token.len() <= MAX_TOKEN;
        if !fits || tunnel_path != tunnel.path() {
            return Err(TunnelError::BadTicket);
        }
        Ok(Self { token, tunnel })
    }
}

impl fmt::Debug for Ticket {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Ticket({}, hidden)", self.tunnel.path())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Frame {
    Text(String),
    Binary(Vec<u8>),
    Alive,
    Closed(Option<u16>),
}

/// No variant holds the URL, the ticket or a library's error text.
#[derive(Debug, thiserror::Error)]
pub enum TunnelError {
    #[error("The hub gave a tunnel ticket this program cannot use.")]
    BadTicket,
    #[error("The hub refused the tunnel (HTTP status {status}).")]
    Refused { status: u16 },
    #[error("The tunnel to {origin} could not be opened ({reason}).")]
    Unreachable { origin: String, reason: String },
    #[error("The tunnel to the hub stopped taking data.")]
    Stalled,
    #[error("The tunnel to the hub broke.")]
    Broken,
}

impl From<TunnelError> for HubError {
    fn from(error: TunnelError) -> Self {
        let (code, message) = match &error {
            TunnelError::Unreachable { origin, reason } => {
                return Self::Unreachable {
                    origin: origin.clone(),
                    reason: reason.clone(),
                };
            }
            TunnelError::Refused { status: 403 } => (
                Code::Forbidden,
                "The hub refused the tunnel ticket: it is used, expired, or from another address."
                    .to_owned(),
            ),
            TunnelError::Refused { status: 503 } => (
                Code::Unavailable,
                "The hub could not open SSH to the device (unreachable, or no accepted host key)."
                    .to_owned(),
            ),
            TunnelError::Stalled | TunnelError::Broken => (Code::Unavailable, error.to_string()),
            TunnelError::BadTicket | TunnelError::Refused { .. } => {
                (Code::Internal, error.to_string())
            }
        };
        Self::Refused {
            code,
            message,
            fields: Vec::new(),
        }
    }
}

pub struct Socket {
    inner: WebSocketStream<Box<dyn Wire>>,
}

impl Hub {
    /// `wait` covers the hub opening SSH before it answers.
    ///
    /// # Errors
    /// [`TunnelError`]: no connection, a refusal before the upgrade, or the wrong subprotocol.
    pub async fn open_tunnel(&self, ticket: Ticket, wait: Duration) -> Result<Socket, TunnelError> {
        let url = self.origin().graphql();
        let unreachable = |reason: String| TunnelError::Unreachable {
            origin: self.origin().as_text(),
            reason,
        };
        let opening = async {
            let wire = wire::connect(&url).await.map_err(unreachable)?;
            upgrade(&url, &ticket, wire).await
        };
        match timeout(wait, opening).await {
            Ok(opened) => opened,
            Err(_) => Err(unreachable(format!(
                "it took longer than {} seconds",
                wait.as_secs()
            ))),
        }
    }
}

async fn upgrade(url: &Url, ticket: &Ticket, wire: Box<dyn Wire>) -> Result<Socket, TunnelError> {
    let scheme = if url.scheme() == "https" { "wss" } else { "ws" };
    // The origin has no user name (CMD-013), so the authority is the host and any port.
    let authority = url.authority();
    let path = ticket.tunnel.path();
    let address = Zeroizing::new(format!(
        "{scheme}://{authority}{path}?token={}",
        ticket.token.as_str()
    ));
    let mut request = address
        .as_str()
        .into_client_request()
        .map_err(|_| TunnelError::BadTicket)?;
    let subprotocol = ticket.tunnel.subprotocol();
    request.headers_mut().insert(
        SEC_WEBSOCKET_PROTOCOL,
        HeaderValue::from_static(subprotocol),
    );
    let config = WebSocketConfig::default()
        .max_message_size(Some(ticket.tunnel.max_message()))
        .max_frame_size(Some(ticket.tunnel.max_message()));
    let (inner, response) =
        tokio_tungstenite::client_async_with_config(request, wire, Some(config))
            .await
            .map_err(|error| match error {
                WsError::Http(response) => TunnelError::Refused {
                    status: response.status().as_u16(),
                },
                _ => TunnelError::Broken,
            })?;
    let agreed = response.headers().get(SEC_WEBSOCKET_PROTOCOL);
    if agreed.and_then(|value| value.to_str().ok()) != Some(subprotocol) {
        return Err(TunnelError::Broken);
    }
    Ok(Socket { inner })
}

impl Socket {
    /// The next message. Cancel safe: a message is taken whole or not at all.
    ///
    /// # Errors
    /// [`TunnelError::Broken`] when the connection fails or the hub breaks the protocol.
    pub async fn recv(&mut self) -> Result<Frame, TunnelError> {
        match self.inner.next().await {
            None | Some(Err(WsError::ConnectionClosed | WsError::AlreadyClosed)) => {
                Ok(Frame::Closed(None))
            }
            Some(Ok(Message::Text(text))) => Ok(Frame::Text(text.as_str().to_owned())),
            Some(Ok(Message::Binary(bytes))) => Ok(Frame::Binary(bytes.to_vec())),
            Some(Ok(Message::Ping(_) | Message::Pong(_) | Message::Frame(_))) => Ok(Frame::Alive),
            Some(Ok(Message::Close(frame))) => {
                Ok(Frame::Closed(frame.map(|frame| u16::from(frame.code))))
            }
            Some(Err(_)) => Err(TunnelError::Broken),
        }
    }

    /// # Errors
    /// [`TunnelError::Stalled`] after [`SEND_WAIT`]; [`TunnelError::Broken`].
    pub async fn send_text(&mut self, text: &str) -> Result<(), TunnelError> {
        self.send(Message::text(text)).await
    }

    /// # Errors
    /// As [`Socket::send_text`].
    pub async fn send_binary(&mut self, bytes: Vec<u8>) -> Result<(), TunnelError> {
        self.send(Message::binary(bytes)).await
    }

    async fn send(&mut self, message: Message) -> Result<(), TunnelError> {
        match timeout(SEND_WAIT, self.inner.send(message)).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => Err(TunnelError::Broken),
            Err(_) => Err(TunnelError::Stalled),
        }
    }

    pub async fn close(mut self) {
        let frame = CloseFrame {
            code: CloseCode::Normal,
            reason: "".into(),
        };
        let closing = async {
            if self.inner.close(Some(frame)).await.is_ok() {
                while let Some(Ok(_)) = self.inner.next().await {}
            }
        };
        let _closed = timeout(CLOSE_WAIT, closing).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "AbCdEfGhIjKlMnOpQrStUvWxYz0123456789-_abcde";

    #[test]
    fn api_009_a_ticket_never_shows_its_token() {
        let ticket = Ticket::new(TOKEN.to_owned(), "/terminal", Tunnel::Terminal).expect("ticket");
        let shown = format!("{ticket:?} {ticket:#?}");
        assert!(!shown.contains(TOKEN), "{shown}");
        assert!(shown.contains("/terminal"), "{shown}");
    }

    #[test]
    fn api_009_a_ticket_is_for_its_own_path_and_is_plain_base64url() {
        assert!(Ticket::new(TOKEN.to_owned(), "/files", Tunnel::Files).is_ok());
        for (token, path, tunnel) in [
            (TOKEN, "/files", Tunnel::Terminal),
            (TOKEN, "/terminal/../x", Tunnel::Terminal),
            (TOKEN, "//evil.example/terminal", Tunnel::Terminal),
            ("a&hub=x", "/terminal", Tunnel::Terminal),
            ("a b", "/terminal", Tunnel::Terminal),
            ("", "/terminal", Tunnel::Terminal),
        ] {
            let refused = Ticket::new(token.to_owned(), path, tunnel);
            assert!(
                matches!(refused, Err(TunnelError::BadTicket)),
                "{token} {path}"
            );
        }
        let long = "a".repeat(MAX_TOKEN + 1);
        assert!(Ticket::new(long, "/terminal", Tunnel::Terminal).is_err());
    }

    #[test]
    fn cmd_014_tunnel_errors_name_no_ticket() {
        let errors = [
            TunnelError::BadTicket,
            TunnelError::Refused { status: 403 },
            TunnelError::Stalled,
            TunnelError::Broken,
        ];
        for error in errors {
            let hub = HubError::from(error);
            assert!(!hub.to_string().contains("token"), "{hub}");
        }
        let forbidden = HubError::from(TunnelError::Refused { status: 403 });
        assert_eq!(forbidden.code(), Code::Forbidden);
    }
}
