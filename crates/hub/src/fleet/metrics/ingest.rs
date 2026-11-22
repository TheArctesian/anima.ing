//! The Anima daemon's side of the hub: enrolment codes, daemon tokens, the `/daemon/v1/` routes
//! and the stale mark of a daemon that stopped pushing (K8, MET-011 to MET-018, R-092 D-1).
//!
//! A code and a token are kept only as the SHA-256 of their random bytes, and each belongs to
//! one device. Neither reaches a log, an event, an error or any response other than the one
//! that hands it out (LOG-007). The core takes the time and the new secret as parameters; the
//! OS random source is read only by the two edges, the routes and the GraphQL mutations.

mod admission;
mod credential;
mod enrolment;
mod keys;
mod push;
mod refusals;
mod renewal;
mod routes;
mod stale;
mod store;
#[cfg(test)]
mod tests;
mod throttle;
mod turns;
mod work;

use chrono::TimeDelta;
use zeroize::Zeroizing;

#[cfg(test)]
use enrolment::make_code;
pub use enrolment::{DaemonEnrolment, enrol_code};
pub(super) use enrolment::{own_device_code, revoke};
pub use routes::daemon_router;
pub use stale::spawn_stale_watch;
pub(super) use stale::without_fresh_daemon;
pub(super) use store::{daemon_needs, daemon_versions};

use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use crate::{
    fleet::{agent_usage, devices, events},
    ids::DeviceId,
};

/// How long an enrolment code works (MET-012).
const CODE_LIFETIME: TimeDelta = TimeDelta::minutes(15);

/// How long a daemon token works, from enrolment or renewal (MET-013).
const TOKEN_LIFETIME: TimeDelta = TimeDelta::days(365);

/// A batch this recent shows a daemon that holds its token: a need reported for it was read
/// before the person allowed it, and is not set (R-364).
const FRESH_BATCH: TimeDelta = TimeDelta::seconds(5);

/// The event of a daemon that waited for a person and sends again, or that a new token replaced
/// (R-364).
fn need_gone(device: DeviceId, name: &str) -> events::NewEvent {
    events::NewEvent {
        severity: events::Severity::Info,
        source: events::Source::Metrics,
        device: Some((device, name.to_owned())),
        message: format!("The daemon on {name} no longer waits for a person."),
        actor: events::ActorKind::Hub,
    }
}

/// Sets the need of the device's daemon at `now` in the caller's transaction; a need is not set
/// when a batch came within [`FRESH_BATCH`] before `now`. `None` when the device has no daemon
/// token, else whether the need changed (R-364).
///
/// # Errors
/// The database error when the write fails.
pub(super) async fn set_need(
    conn: &mut PgConnection,
    device: DeviceId,
    need: Option<&str>,
    now: DateTime<Utc>,
) -> Result<Option<bool>, sqlx::Error> {
    store::set_need(conn, device, need, now - FRESH_BATCH).await
}

/// Why a daemon operation failed. Messages are fixed text: they hold no code, token, body or
/// SQL (LOG-007).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("the event log cannot be written")]
    Events(#[from] events::Error),
    #[error("the OS random source failed")]
    Random(#[source] getrandom::Error),
    /// The device was deleted while its daemon enrolled.
    #[error("the device does not exist")]
    NoDevice,
    #[error("the device cannot be read")]
    Devices(#[source] devices::Error),
    #[error("the metrics cannot be stored")]
    Metrics(#[from] super::Error),
    #[error("the agent usage cannot be stored")]
    AgentUsage(#[from] agent_usage::Error),
    #[error("the operational key cannot be given")]
    Keys(#[source] crate::fleet::credentials::operational::Error),
    #[error("the job cannot be given or changed")]
    Jobs(#[source] crate::work::jobs::Error),
}

impl From<devices::Error> for Error {
    fn from(error: devices::Error) -> Self {
        match error {
            devices::Error::NotFound(_) => Self::NoDevice,
            other => Self::Devices(other),
        }
    }
}

/// `N` bytes from the OS random source, wiped on drop (R-092 D-1).
///
/// # Errors
/// `Random` when the source fails.
pub(super) fn random<const N: usize>() -> Result<Zeroizing<[u8; N]>, Error> {
    let mut bytes = Zeroizing::new([0_u8; N]);
    getrandom::fill(bytes.as_mut_slice()).map_err(Error::Random)?;
    Ok(bytes)
}
