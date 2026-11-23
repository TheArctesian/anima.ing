//! The hub's client of the VPN broker (R-055, VP-7, BD-10, BD-15). The reply is hostile input:
//! read to at most 64 KiB, parsed once, never echoed into an error, as it can hold a key (LOG-007).

use std::{
    fmt,
    io::ErrorKind,
    net::{IpAddr, Ipv4Addr},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

#[cfg(windows)]
use anima_windows::socket::Stream as UnixStream;
use chrono::{DateTime, Utc};
use ipnet::IpNet;
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use serde_json::{Value, json};
#[cfg(unix)]
use tokio::net::UnixStream;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    sync::{Mutex, MutexGuard},
    time::Instant,
};
use zeroize::Zeroizing;

use super::Error;

/// Without the newline (BD-10).
pub(super) const MAX_REPLY_BYTES: usize = 64 * 1024;
/// `flock -w 8` in `deploy/headscale/broker/anima-broker@.unit`.
pub(super) const BROKER_LOCK_WAIT: Duration = Duration::from_secs(8);
/// `READ_MS` in `broker/main.ts`.
pub(super) const BROKER_REQUEST_READ: Duration = Duration::from_secs(5);
/// `REQUEST_MS` in `broker/main.ts`.
pub(super) const BROKER_REQUEST_DEADLINE: Duration = Duration::from_secs(45);
/// `RuntimeMaxSec=60` in the unit.
pub(super) const BROKER_RUNTIME_MAX: Duration = Duration::from_secs(60);
const REPLY_MARGIN: Duration = Duration::from_secs(5);
/// Outlives any broker request, so a change it made is never reported as unanswered and the
/// turn is never let go while it runs (BD-10).
pub(super) const CALL_LIMIT: Duration =
    Duration::from_secs(BROKER_RUNTIME_MAX.as_secs() + REPLY_MARGIN.as_secs());
const _: () = assert!(
    BROKER_LOCK_WAIT.as_secs() + BROKER_REQUEST_READ.as_secs() + BROKER_REQUEST_DEADLINE.as_secs()
        <= BROKER_RUNTIME_MAX.as_secs()
);
/// Then the call gives up with `BrokerBusy`, having sent nothing (BD-15).
pub(super) const TURN_WAIT: Duration = Duration::from_secs(20);
pub(super) const CLOSE_WAIT: Duration = Duration::from_secs(2);

/// Clones share one turn, held until the broker closes, so no two calls reach the socket at
/// once: systemd's `MaxConnections=1` closes a second connection at once (BD-15).
#[derive(Clone, Debug)]
pub struct Broker {
    path: PathBuf,
    turn: Arc<Mutex<()>>,
}

/// Holds no key (BD-9).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub(super) struct BrokerNode {
    pub id: u64,
    pub name: String,
    pub user: String,
    pub tags: Vec<String>,
    pub ips: Vec<IpAddr>,
    pub online: bool,
    pub expiry: Option<DateTime<Utc>>,
    pub approved: Vec<IpNet>,
    pub available: Vec<IpNet>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(from = "NodesReply")]
pub(super) struct BrokerView {
    pub nodes: Vec<BrokerNode>,
    pub sites: Vec<(String, IpNet)>,
}

#[derive(Deserialize)]
struct NodesReply {
    nodes: Vec<BrokerNode>,
    sites: Vec<PolicySite>,
}

#[derive(Deserialize)]
struct PolicySite {
    name: String,
    subnet: IpNet,
}

impl From<NodesReply> for BrokerView {
    fn from(reply: NodesReply) -> Self {
        Self {
            nodes: reply.nodes,
            sites: reply
                .sites
                .into_iter()
                .map(|site| (site.name, site.subnet))
                .collect(),
        }
    }
}

/// No `Display`, a `Debug` without the key, and zeroed when it drops (LOG-007).
pub(super) struct PreAuthKey(Zeroizing<String>);

impl PreAuthKey {
    /// Only for the one reply that hands the key to the operator.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PreAuthKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[pre-auth key]")
    }
}

impl<'de> Deserialize<'de> for PreAuthKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(|key| Self(Zeroizing::new(key)))
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct NewKey {
    pub key: PreAuthKey,
    #[serde(rename = "expiresAt")]
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Registered {
    pub name: String,
    pub address: Ipv4Addr,
    #[serde(rename = "keyExpiry")]
    pub key_expiry: Option<DateTime<Utc>>,
}

/// The words of the broker's wire format.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Refusal {
    BadRequest,
    UnknownSite,
    RateLimited,
    NodeLimit,
    NotAllowed,
    Undone,
    HeadscaleFailed,
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BadRequest => "bad-request",
            Self::UnknownSite => "unknown-site",
            Self::RateLimited => "rate-limited",
            Self::NodeLimit => "node-limit",
            Self::NotAllowed => "not-allowed",
            Self::Undone => "undone",
            Self::HeadscaleFailed => "headscale-failed",
        })
    }
}

/// One reply line: `{"ok":true,"value":…}` or `{"ok":false,"refusal":…,"detail":…}`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply<T> {
    ok: bool,
    value: Option<T>,
    refusal: Option<Refusal>,
    #[serde(default)]
    detail: String,
}

impl Broker {
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            turn: Arc::new(Mutex::new(())),
        }
    }

    /// `BrokerBusy` at once when the turn is held, so a reader never queues behind a mutation
    /// (BD-15).
    pub(super) async fn nodes_if_free(&self) -> Result<BrokerView, Error> {
        let turn = self.turn.try_lock().map_err(|_| Error::BrokerBusy)?;
        self.call_holding(turn, &json!({ "verb": "nodes" })).await
    }

    /// A one-use, one-hour key for an untagged node of the user `operator` (VPN-003).
    pub(super) async fn personal_key(&self) -> Result<NewKey, Error> {
        self.call(&json!({ "verb": "personal-key" })).await
    }

    /// A one-use, one-hour key tagged for the router of the policy site `site` (VPN-001).
    pub(super) async fn router_key(&self, site: &str) -> Result<NewKey, Error> {
        self.call(&json!({ "verb": "router-key", "site": site }))
            .await
    }

    /// Registers the node that waits with `auth_id` to the user `operator` (vpn.md 6B).
    pub(super) async fn register(&self, auth_id: &str) -> Result<Registered, Error> {
        self.call(&json!({ "verb": "register", "authId": auth_id }))
            .await
    }

    pub(super) async fn approve_route(&self, node: u64, site: &str) -> Result<(), Error> {
        let request = json!({ "verb": "approve-route", "node": node, "site": site });
        self.call::<serde::de::IgnoredAny>(&request).await.map(drop)
    }

    /// Sets a staggered key expiry on the untagged operator node `node` (BD-17, VP-10).
    pub(super) async fn renew(&self, node: u64) -> Result<Registered, Error> {
        self.call(&json!({ "verb": "renew", "node": node })).await
    }

    async fn call<T: DeserializeOwned>(&self, request: &Value) -> Result<T, Error> {
        let turn = tokio::time::timeout(TURN_WAIT, self.turn.lock())
            .await
            .map_err(|_| Error::BrokerBusy)?;
        self.call_holding(turn, request).await
    }

    /// Holds the turn from connect to close.
    async fn call_holding<T: DeserializeOwned>(
        &self,
        _turn: MutexGuard<'_, ()>,
        request: &Value,
    ) -> Result<T, Error> {
        let deadline = Instant::now() + CALL_LIMIT;
        let (line, stream) = tokio::time::timeout_at(deadline, self.exchange(request))
            .await
            .map_err(|_| Error::BrokerTimedOut)??;
        // The reply is in hand: a broker that does not close in time fails nothing.
        let closed_by = deadline.min(Instant::now() + CLOSE_WAIT);
        let _ = tokio::time::timeout_at(closed_by, closed(stream)).await;
        parse_reply(&line)
    }

    async fn exchange(&self, request: &Value) -> Result<(Zeroizing<Vec<u8>>, UnixStream), Error> {
        let mut stream =
            UnixStream::connect(&self.path)
                .await
                .map_err(|error| match error.kind() {
                    ErrorKind::NotFound | ErrorKind::ConnectionRefused => Error::NotInstalled,
                    _ => Error::BrokerConnection,
                })?;
        let mut out = request.to_string().into_bytes();
        out.push(b'\n');
        stream
            .write_all(&out)
            .await
            .map_err(|_| Error::BrokerConnection)?;
        let line = read_line(&mut stream).await?;
        Ok((line, stream))
    }
}

async fn closed(mut stream: UnixStream) {
    let mut rest = Zeroizing::new([0_u8; 512]);
    while matches!(stream.read(&mut rest[..]).await, Ok(read) if read > 0) {}
}

/// One buffer of the full size that never grows and is zeroed on drop, so no copy of a key is
/// left (LOG-007).
async fn read_line(stream: &mut UnixStream) -> Result<Zeroizing<Vec<u8>>, Error> {
    let mut line = Zeroizing::new(vec![0_u8; MAX_REPLY_BYTES + 1]);
    let mut filled = 0;
    let end = loop {
        let read = stream
            .read(&mut line[filled..])
            .await
            .map_err(|_| Error::BrokerUnreadable)?;
        let newline = line[filled..filled + read].iter().position(|&b| b == b'\n');
        filled += read;
        match newline {
            Some(at) => break filled - read + at,
            None if read == 0 || filled == line.len() => break filled,
            None => {}
        }
    };
    line.truncate(end);
    if line.is_empty() || line.len() > MAX_REPLY_BYTES {
        return Err(Error::BrokerUnreadable);
    }
    Ok(line)
}

/// A parse error never carries the line's text: serde quotes the value it could not read, and
/// that can be a key (LOG-007).
pub(super) fn parse_reply<T: DeserializeOwned>(line: &[u8]) -> Result<T, Error> {
    let reply: Reply<T> = serde_json::from_slice(line).map_err(|_| Error::BrokerUnreadable)?;
    match (reply.ok, reply.value, reply.refusal) {
        (true, Some(value), None) => Ok(value),
        (false, None, Some(refusal)) => Err(Error::BrokerRefused {
            refusal,
            detail: crate::text::clean(&reply.detail, DETAIL_CHARS),
        }),
        _ => Err(Error::BrokerUnreadable),
    }
}

const DETAIL_CHARS: usize = 160;

/// The picture matches nodes by their first IPv4 address (BD-14).
pub(super) fn ipv4_of(node: &BrokerNode) -> Option<Ipv4Addr> {
    node.ips.iter().find_map(|ip| match ip {
        IpAddr::V4(v4) => Some(*v4),
        IpAddr::V6(_) => None,
    })
}
