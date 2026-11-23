//! The one interface between the proxy and the hub: JSON lines on a hub socket (Section 12.3, SF-231).
//! No request names a device, an address or a port: the hub finds them in its own record.

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufRead, AsyncBufReadExt as _, AsyncWrite, AsyncWriteExt as _};
use uuid::Uuid;
use zeroize::Zeroizing;

/// The longest line of a request; a table line can be longer (each certificate chain).
pub const REQUEST_MAX: usize = 4 * 1024;
/// The longest line of a table.
pub const TABLE_MAX: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    /// The names that serve now, with their certificates, and the streams.
    Table,
    /// A connection to the port of a route of this application.
    Connect(Dial),
    /// What the proxy counted for one name since its last report.
    Count { name: String, counts: Counts },
}

/// A connection that the proxy asks for: the application and the name of its route.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dial {
    pub application: Uuid,
    pub name: String,
    #[serde(default)]
    pub transport: Transport,
}

/// How the bytes of a route go. After `ok`, a UDP connection carries datagrams as frames
/// (`crate::streams::datagrams`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    #[default]
    Tcp,
    Udp,
}

/// Who may reach a route (Section 16.7): the internet, or only the nodes of the network of
/// Anima on the private address of the hub.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reach {
    #[default]
    Public,
    Private,
}

/// What the proxy counts for a name (Section 12.7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Counts {
    pub requests: u64,
    pub bytes_in: u64,
    pub bytes_out: u64,
    /// Connections that a limit refused.
    pub refused: u64,
}

/// The table of names. No `Debug`: it holds private keys (LOG-007).
#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    pub names: Vec<Named>,
    /// The open ACME `HTTP-01` challenges: each token with its key authorization.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub challenges: Vec<Challenge>,
    /// The streams that serve now: a port of the hub each.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub streams: Vec<StreamRoute>,
}

/// One stream that serves: the proxy listens on its port and connects by its name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StreamRoute {
    pub name: String,
    pub application: Uuid,
    pub transport: Transport,
    pub listen_port: u16,
    pub reach: Reach,
}

/// One open ACME challenge that the proxy answers on port 80.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Challenge {
    pub token: String,
    pub key_authorization: String,
}

/// One name that serves, with its certificate. No `Debug`: it holds the private key.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Named {
    pub name: String,
    pub application: Uuid,
    pub chain_pem: String,
    pub key_pem: Zeroizing<String>,
    #[serde(default)]
    pub reach: Reach,
    /// The one `https://` address of a redirect; `None` for a web route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect_to: Option<String>,
}

/// The answer to `connect`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// The bytes of the application follow.
    Ok,
    /// No route of this application serves, or a limit of its link refused.
    Refused,
    /// The machine did not answer.
    Unavailable,
}

impl Answer {
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Refused => "refused",
            Self::Unavailable => "unavailable",
        }
    }

    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        [Self::Ok, Self::Refused, Self::Unavailable]
            .into_iter()
            .find(|answer| answer.word() == word)
    }
}

/// Why a line of the interface was not read.
#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("the line is longer than its limit")]
    TooLong,
    #[error("the line is not a request or an answer of the interface")]
    Malformed,
    #[error("the socket of the interface failed")]
    Io(#[from] std::io::Error),
}

/// Reads one line of at most `max` bytes, without its end. `None` at the end of the stream.
///
/// # Errors
/// [`WireError::TooLong`] or the I/O error.
pub async fn read_line<R: AsyncBufRead + Unpin>(
    reader: &mut R,
    max: usize,
) -> Result<Option<Zeroizing<String>>, WireError> {
    let mut line = Zeroizing::new(Vec::new());
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return (!line.is_empty()).then(|| text(&line)).transpose();
        }
        let end = available.iter().position(|byte| *byte == b'\n');
        let take = end.map_or(available.len(), |at| at + 1);
        if line.len() + take > max + 1 {
            return Err(WireError::TooLong);
        }
        line.extend_from_slice(&available[..take]);
        reader.consume(take);
        if end.is_some() {
            line.pop();
            return text(&line).map(Some);
        }
    }
}

fn text(bytes: &[u8]) -> Result<Zeroizing<String>, WireError> {
    std::str::from_utf8(bytes)
        .map(|text| Zeroizing::new(text.to_owned()))
        .map_err(|_| WireError::Malformed)
}

/// Writes `value` as one JSON line.
///
/// # Errors
/// The I/O error.
pub async fn write_json<W: AsyncWrite + Unpin, T: Serialize>(
    writer: &mut W,
    value: &T,
) -> Result<(), WireError> {
    let mut line = Zeroizing::new(serde_json::to_vec(value).map_err(|_| WireError::Malformed)?);
    line.push(b'\n');
    writer.write_all(&line).await?;
    writer.flush().await?;
    Ok(())
}

/// # Errors
/// [`WireError::Malformed`] for any other text.
pub fn parse_request(line: &str) -> Result<Request, WireError> {
    serde_json::from_str(line).map_err(|_| WireError::Malformed)
}
