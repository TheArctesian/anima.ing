//! Remote sessions and the single-use tickets that open a tunnel (RA-008, K4 §1, API-009).
//! SSH sessions are pinned to the host key `trust` holds (SEC-006, SEC-007).

mod admission;
mod connection_test;
mod end;
mod files;
pub mod graphql;
mod live;
mod model;
mod page;
mod quiet;
mod serving;
mod start;
mod store;
mod terminal;
#[cfg(test)]
mod tests;
mod tickets;
mod tunnel;
mod upgrade;
mod wake;

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::{
    api_errors::field_names,
    engine::locations,
    fleet::{
        credentials,
        devices::{self, Device},
        events, trust,
    },
    ids::{DeviceId, SessionId},
};

use model::Desktop;

pub use end::Actor;
pub(crate) use end::DeviceSessions;
pub use files::{FilesMutation, files_router};
pub use live::{EndCause, Live, Redeemed, Refused, Tunnel};
pub use model::{
    EndReason, FieldError, Protocol, Session, SessionChange, SessionEnd, UnknownValue,
};
pub use serving::Serving;
pub use start::{DEFAULT_DPI, DisplayRequest, StartRequest, Waking, start};
pub use terminal::terminal_router;
pub use tickets::{PendingTicket, Presented, Ticket};
pub use tunnel::tunnel_router;
pub use wake::{Error as WakeError, WakeSent, graphql::Mutation as WakeMutation, wake};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Missing {
    Device(DeviceId),
    Session(SessionId),
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Device(id) => write!(f, "device {id}"),
            Self::Session(id) => write!(f, "session {id}"),
        }
    }
}

/// Why the device's state does not allow a session now (K4 §1, §6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Conflict {
    #[error("SSH is turned off for this device. Turn it on in the device's SSH settings.")]
    SshDisabled,
    #[error("No host key is accepted for this device. Test the connection and accept its key.")]
    NoHostKey,
    #[error("RDP is turned off for this device. Turn it on in the device's RDP settings.")]
    RdpDisabled,
    #[error("VNC is turned off for this device. Turn it on in the device's VNC settings.")]
    VncDisabled,
    /// RDP checks the server certificate, and none is accepted yet (SEC-007).
    #[error(
        "No server certificate is accepted for this device. Test the connection and accept \
         its certificate."
    )]
    NoServerCertificate,
    /// The device has as many live sessions as its limit allows (RA-036).
    #[error("This device has reached its session limit.")]
    SessionLimit,
    /// The stored credential was entered for another host; nothing was sent (SEC-011).
    #[error("{}", credential_moved(*.0))]
    CredentialMoved(Protocol),
}

pub(in crate::fleet::sessions) const fn credential_moved(protocol: Protocol) -> &'static str {
    match protocol {
        Protocol::Ssh => {
            "The device's address changed after its SSH credential was entered, so the hub \
             sent no credential. Enter the credential again in the device's SSH settings."
        }
        Protocol::Rdp => {
            "The device's address changed after its RDP password was entered, so the hub sent \
             no password. Enter the password again in the device's RDP settings."
        }
        Protocol::Vnc => {
            "The device's address changed after its VNC password was entered, so the hub sent \
             no password. Enter the password again in the device's VNC settings."
        }
    }
}

/// Why a sessions operation failed. No variant holds a ticket token (LOG-007).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} not found")]
    NotFound(Missing),
    #[error("input is not valid: {}", field_names(.0))]
    Validation(Vec<FieldError>),
    #[error(transparent)]
    Conflict(Conflict),
    #[error("no session can start now")]
    Unavailable,
    /// The messages stay generic: the source errors may hold SQL text (API-010).
    #[error("device store error")]
    Devices(#[source] devices::Error),
    #[error("event store error")]
    Events(#[from] events::Error),
    #[error("credential store error")]
    Credentials(#[source] credentials::Error),
    #[error("trust store error")]
    Trust(#[from] trust::Error),
    #[error("no random bytes for a ticket")]
    Random(#[source] getrandom::Error),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    /// The context of a transfer event could not be stored (K-E4 Section 12).
    #[error("transfer context store error")]
    Context(#[from] locations::Error),
}

async fn find_device(db: &PgPool, id: DeviceId) -> Result<Device, Error> {
    devices::get(db, id)
        .await
        .map_err(|error| device_error(id, error))
}

fn device_error(id: DeviceId, error: devices::Error) -> Error {
    match error {
        devices::Error::NotFound(_) => Error::NotFound(Missing::Device(id)),
        other => Error::Devices(other),
    }
}

/// Ends every session a previous hub process left open, with an audit entry and no event (D-13).
/// Call it once at start, before the hub serves.
///
/// # Errors
/// `Database` or `Events` when the rows or audit entries cannot be written; then none is.
pub async fn close_stale(db: &PgPool, now: DateTime<Utc>) -> Result<u64, Error> {
    end::close_stale(db, now).await
}

/// Ends what a previous process left open before it returns the `Live` that issues tickets (R-040, D-13).
///
/// # Errors
/// As [`close_stale`]; then no `Live` is returned.
pub async fn open(db: &PgPool, now: DateTime<Utc>) -> Result<(Live, u64), Error> {
    let ended = end::close_stale(db, now).await?;
    Ok((Live::default(), ended))
}

/// Call on graceful shutdown: tunnels are upgraded connections, which the server's drain does not end (D-10).
/// What it misses within the tunnel end wait, the next start's [`close_stale`] ends.
///
/// # Errors
/// `Database` or `Events` when a pending ticket's row or audit entry cannot be written.
pub async fn shut_down(db: &PgPool, live: &Live, now: DateTime<Utc>) -> Result<(), Error> {
    end::shut_down(db, live, now).await
}
