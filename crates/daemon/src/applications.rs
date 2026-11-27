//! Enrolled daemon side of the local hosting control protocol.
mod apply;

pub(crate) use apply::{ApplyFiles, reason as apply_failure};
use std::{path::Path, time::Duration};

use anima_applications_wire::{
    Command, Decision, MAX_REPLY_BYTES, MAX_REQUEST_BYTES, Message, Reply, SOCKET,
};
use chrono::Utc;
use daemon_core::wire::{Held, Reported};
use serde::Serialize;
use serde_json::Value;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::UnixStream,
    time,
};
use uuid::Uuid;

use crate::config::origin::{Origin, Reach};

const CONNECT: Duration = Duration::from_secs(3);
const PREPARATION: Duration = Duration::from_secs(24);
const OPERATION: Duration = Duration::from_secs(123);

/// The same enrolled identity sent with the probe and with each local item.
pub struct Client {
    origin: String,
    device: String,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the applications socket is unavailable")]
    Connection,
    #[error("the applications socket peer is not root")]
    Owner,
    #[error("the applications socket sent an invalid result")]
    Protocol,
    #[error("the hub did not renew the lease for a start")]
    NotAuthorized,
}

/// A policy refusal is returned before the caller reports the item as preparing.
pub enum Offer {
    Ready(Prepared),
    Refused(String),
    Failed(String),
}

/// An offer accepted by the applications owner. Dropping it leaves Docker unchanged.
pub struct Prepared {
    item: Held,
    stream: UnixStream,
}

/// Final local outcome after the daemon grants a start.
pub enum Outcome {
    Succeeded,
    Failed(String),
}

impl Client {
    /// Creates the K10 pair for an HTTPS enrollment, with the implicit port written out.
    #[must_use]
    pub fn enrolled(origin: &Origin, device: Uuid) -> Option<Self> {
        let Reach::Https(url) = origin.reach() else {
            return None;
        };
        let host = url.host_str()?;
        (!device.is_nil()).then(|| Self {
            origin: format!("https://{host}:443"),
            device: device.to_string(),
        })
    }

    /// Check the current pairing before the daemon advertises an application capability.
    pub async fn paired(&self) -> bool {
        matches!(
            self.exchange(Command::Probe, CONNECT).await,
            Ok((_, Reply::Paired))
        )
    }

    /// Ask the applications owner to inspect and hold a plan without starting it.
    ///
    /// # Errors
    /// The socket is unavailable, owned by another user, or violates the framing protocol.
    pub async fn prepare(&self, item: &Value) -> Result<Offer, Error> {
        let identity = identity(item)?;
        let (stream, reply) = self.offer(item).await?;
        match reply {
            Reply::Ready => Ok(Offer::Ready(Prepared {
                item: identity,
                stream,
            })),
            Reply::Refused { cause } => Ok(Offer::Refused(container_refusal(&cause).to_owned())),
            Reply::Failed { cause } => Ok(Offer::Failed(failure(&cause).to_owned())),
            _ => Err(Error::Protocol),
        }
    }

    async fn offer(&self, item: &Value) -> Result<(UnixStream, Reply), Error> {
        let json = serde_json::to_string(item).map_err(|_| Error::Protocol)?;
        if json.len() > anima_applications_wire::MAX_ITEM_BYTES {
            return Err(Error::Protocol);
        }
        self.exchange(Command::Item { json }, PREPARATION).await
    }

    async fn exchange(
        &self,
        command: Command,
        deadline: Duration,
    ) -> Result<(UnixStream, Reply), Error> {
        let mut stream = time::timeout(CONNECT, root_peer(Path::new(SOCKET)))
            .await
            .map_err(|_| Error::Connection)??;
        let message = Message {
            origin: self.origin.clone(),
            device: self.device.clone(),
            command,
        };
        let reply = time::timeout(deadline, async {
            send(&mut stream, &message, MAX_REQUEST_BYTES).await?;
            read(&mut stream).await
        })
        .await
        .map_err(|_| Error::Connection)??;
        Ok((stream, reply))
    }
}

impl Prepared {
    /// The one attempt this socket can start.
    #[must_use]
    pub const fn item(&self) -> Held {
        self.item
    }

    /// Send the start only after the hub accepted the running report for this attempt.
    ///
    /// # Errors
    /// The hub stopped the item, its lease expired, or the socket returned an invalid result.
    /// A transport error after the start is ambiguous and the caller must never resend the item.
    pub(crate) async fn start(mut self, acknowledgement: Reported) -> Result<Outcome, Error> {
        if acknowledgement.stop.is_some() || acknowledgement.lease.until <= Utc::now() {
            return Err(Error::NotAuthorized);
        }
        time::timeout(OPERATION, async {
            send(&mut self.stream, &Decision::Start, MAX_REPLY_BYTES).await?;
            match read(&mut self.stream).await? {
                Reply::Succeeded => Ok(Outcome::Succeeded),
                Reply::Failed { cause } => Ok(Outcome::Failed(failure(&cause).to_owned())),
                _ => Err(Error::Protocol),
            }
        })
        .await
        .map_err(|_| Error::Connection)?
    }
}

pub(crate) fn identity(item: &Value) -> Result<Held, Error> {
    if !matches!(
        item["type"].as_str(),
        Some("container_action" | "application_apply")
    ) || item["kind"] != "host"
        || item["attempt"] != 1
    {
        return Err(Error::Protocol);
    }
    let id = item["id"]
        .as_str()
        .and_then(|id| Uuid::parse_str(id).ok())
        .filter(|id| !id.is_nil())
        .ok_or(Error::Protocol)?;
    Ok(Held { id, attempt: 1 })
}

async fn root_peer(path: &Path) -> Result<UnixStream, Error> {
    let stream = UnixStream::connect(path)
        .await
        .map_err(|_| Error::Connection)?;
    if stream.peer_cred().map_err(|_| Error::Owner)?.uid() != 0 {
        return Err(Error::Owner);
    }
    Ok(stream)
}

async fn send(stream: &mut UnixStream, value: &impl Serialize, limit: usize) -> Result<(), Error> {
    let bytes = serde_json::to_vec(value).map_err(|_| Error::Protocol)?;
    frame(stream, &bytes, limit).await
}

async fn frame(stream: &mut UnixStream, bytes: &[u8], limit: usize) -> Result<(), Error> {
    let length = u32::try_from(bytes.len()).map_err(|_| Error::Protocol)?;
    if bytes.len() > limit {
        return Err(Error::Protocol);
    }
    stream
        .write_u32(length)
        .await
        .map_err(|_| Error::Connection)?;
    stream.write_all(bytes).await.map_err(|_| Error::Connection)
}

pub(crate) async fn read(stream: &mut (impl tokio::io::AsyncRead + Unpin)) -> Result<Reply, Error> {
    let length = stream.read_u32().await.map_err(|_| Error::Connection)? as usize;
    if length == 0 || length > MAX_REPLY_BYTES {
        return Err(Error::Protocol);
    }
    let mut bytes = vec![0; length];
    stream
        .read_exact(&mut bytes)
        .await
        .map_err(|_| Error::Connection)?;
    serde_json::from_slice(&bytes).map_err(|_| Error::Protocol)
}

fn container_refusal(cause: &str) -> &'static str {
    match cause {
        "applications_off" => "applications_off",
        "busy" => "busy",
        "no_container" => "no_container",
        "not_adopted" => "not_adopted",
        "migration_held" => "migration_held",
        "protected" => "protected",
        "start_not_permitted" => "start_not_permitted",
        "type_unknown" => "type_unknown",
        "kind_not_permitted" => "kind_not_permitted",
        "secret_unknown" => "secret_unknown",
        _ => "member_unknown",
    }
}

fn failure(cause: &str) -> &'static str {
    match cause {
        "busy" => "busy",
        "orphan" => "orphan",
        "limit_seconds" => "limit_seconds",
        "migration_held" => "migration_held",
        _ => "step_failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;
    use daemon_core::wire::{Lease, StopOrder, StopReason};

    #[tokio::test]
    async fn only_a_current_running_acknowledgement_can_start_a_prepared_action() {
        let now = Utc::now();
        let current = Reported {
            lease: Lease {
                until: now + TimeDelta::seconds(90),
            },
            stop: None,
            log_end: None,
        };
        let stopped = Reported {
            stop: Some(StopOrder {
                reason: StopReason::Operator,
            }),
            ..current
        };
        let expired = Reported {
            lease: Lease { until: now },
            ..current
        };
        let item = Held {
            id: Uuid::now_v7(),
            attempt: 1,
        };
        for withheld in [stopped, expired] {
            let (stream, mut owner) = UnixStream::pair().unwrap();
            assert!(matches!(
                Prepared { item, stream }.start(withheld).await,
                Err(Error::NotAuthorized)
            ));
            let mut sent = Vec::new();
            owner.read_to_end(&mut sent).await.unwrap();
            assert_eq!(sent, Vec::<u8>::new());
        }

        let (stream, mut owner) = UnixStream::pair().unwrap();
        let fake_owner = async {
            let length = owner.read_u32().await.unwrap() as usize;
            let mut decision = vec![0; length];
            owner.read_exact(&mut decision).await.unwrap();
            assert!(matches!(
                serde_json::from_slice::<Decision>(&decision).unwrap(),
                Decision::Start
            ));
            send(&mut owner, &Reply::Succeeded, MAX_REPLY_BYTES)
                .await
                .unwrap();
        };
        let (result, ()) = tokio::join!(Prepared { item, stream }.start(current), fake_owner);
        assert!(matches!(result, Ok(Outcome::Succeeded)));
    }
}
