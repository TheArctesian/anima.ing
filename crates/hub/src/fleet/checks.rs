//! Checks of devices, their status and uptime (spec 6.4, HUB-003, INT-001).
//!
//! Other domains use only the contract functions: [`statuses`], [`uptimes`], [`for_device`],
//! [`add_default_check`], [`device_status`], [`resolved_ips`], [`http_urls`] and [`evaluate`].
//!
//! HTTP and SNMP secrets are sealed, bound to their check, and never returned by a read; only
//! [`load_http_secrets`] and [`load_snmp_secrets`] open them, for the probe (SEC-003, LOG-007).
//! SNMP secrets open only for a probe of the device host they were entered for (SEC-010).

pub mod config;
mod evaluation;
mod exposition;
pub mod graphql;
mod model;
pub mod probe;
mod push;
mod rules;
mod scheduler;
mod sealing;
mod status;
mod store;
#[cfg(test)]
mod tests;
mod uptime;

use std::{collections::HashMap, net::IpAddr};

use chrono::{DateTime, Days, Utc};
use sqlx::{Connection as _, PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    api_errors::field_names,
    fleet::{
        credentials::{self, Vault},
        devices, maintenance,
    },
    ids::{CheckId, DeviceId},
    status::Status,
};

use config::{CheckConfig, IpVersion};
use sealing::{prepare_change, sealed};

pub use evaluation::{CheckSettings, CheckState, evaluate};
pub use exposition::exposition_router;
pub(in crate::fleet::checks) use model::Attempt;
pub use model::{
    Beat, Check, CheckChange, CheckDefaults, Definition, Due, FieldError, Heartbeat, HttpSetup,
    Missing, NewCheck, PushToken, Recorded, Sealer, SnmpSetup,
};
pub use push::{OwnCheck, OwnReport, own, push_router};
pub use scheduler::{Probing, spawn_scheduler};
pub use uptime::UptimeWindow;

/// DB-004.
const PARTITION_DAYS_AHEAD: u32 = 7;

/// Why a checks operation failed. Messages hold no SQL, no input value and no secret.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0} not found")]
    NotFound(Missing),
    #[error("input is not valid: {}", field_names(.0))]
    Validation(Vec<FieldError>),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    /// The source names no key or plaintext.
    #[error("the check secrets cannot be used")]
    Credentials(#[from] credentials::Error),
    #[error("the stored configuration of check {0} cannot be read")]
    Unreadable(CheckId),
    /// SEC-005.
    #[error("the stored secrets do not match the check's address")]
    SecretsUnbound,
    /// SEC-010.
    #[error("the stored SNMP secrets were entered for another device host")]
    SecretsForOtherHost,
    /// The database detail, which holds the token, is dropped (LOG-007).
    #[error("the push token is already in use")]
    PushTokenTaken,
    #[error("the devices of the check cannot be read")]
    Devices(#[source] devices::Error),
    #[error("the maintenance windows cannot be read")]
    Maintenance(#[source] maintenance::Error),
    #[error("the OS random source failed")]
    Random(#[source] getrandom::Error),
}

impl Error {
    /// A fixed word for the process log, never the source text (LOG-007, API-010).
    #[must_use]
    pub const fn fault(&self) -> &'static str {
        match self {
            Self::Database(_) => "database",
            _ => "checks",
        }
    }
}

fn from_devices(error: devices::Error) -> Error {
    match error {
        devices::Error::NotFound(devices::Missing::Device(id)) => {
            Error::NotFound(Missing::Device(id))
        }
        other => Error::Devices(other),
    }
}

/// The worst of each device's checks, with pause, groups and maintenance applied (CHK-004,
/// CHK-006); three queries however many devices (API-005).
///
/// # Errors
/// `Database`, `Devices` or `Maintenance` when a read fails.
pub async fn statuses(
    db: &PgPool,
    devices: &[DeviceId],
    now: DateTime<Utc>,
) -> Result<HashMap<DeviceId, Status>, Error> {
    let checks = store::standings(db, devices).await?;
    let known = devices::list(db).await.map_err(from_devices)?;
    let active = maintenance::active_targets(db, now)
        .await
        .map_err(Error::Maintenance)?;
    Ok(status::by_device(devices, &checks, &known, &active))
}

/// CHK-004.
///
/// # Errors
/// `NotFound` when there is no such device.
pub async fn device_status(
    db: &PgPool,
    device: DeviceId,
    now: DateTime<Utc>,
) -> Result<Status, Error> {
    let known = devices::get(db, device).await.map_err(from_devices)?;
    let checks = store::standings(db, &[device]).await?;
    let active = maintenance::active_targets(db, now)
        .await
        .map_err(Error::Maintenance)?;
    let mut shown = status::by_device(&[device], &checks, &[known], &active);
    Ok(shown.remove(&device).unwrap_or(Status::Unknown))
}

/// A device with no counted beats is absent (CHK-011).
///
/// # Errors
/// `Database` when the query fails.
pub async fn uptimes(
    db: &PgPool,
    devices: &[DeviceId],
    window: UptimeWindow,
    now: DateTime<Utc>,
) -> Result<HashMap<DeviceId, f64>, Error> {
    let span = (window.start(now), now);
    let tallies = store::tallies(db, devices, window.resolution(), span).await?;
    Ok(tallies
        .into_iter()
        .filter_map(|(device, tally)| uptime::ratio(tally).map(|ratio| (device, ratio)))
        .collect())
}

/// In byte order of the name.
///
/// # Errors
/// `Unreadable` when a stored configuration cannot be read.
pub async fn for_device(db: &PgPool, device: DeviceId) -> Result<Vec<Check>, Error> {
    store::by_devices(db, &[device]).await
}

/// In one query (API-005); a device with no checks is absent.
///
/// # Errors
/// `Unreadable` when a stored configuration cannot be read.
pub async fn for_devices(
    db: &PgPool,
    devices: &[DeviceId],
) -> Result<HashMap<DeviceId, Vec<Check>>, Error> {
    store::grouped_by_devices(db, devices).await
}

/// The id and URL of each device's HTTP checks, in one query (API-005, WEB-002); a device with
/// none is absent.
///
/// # Errors
/// `Database` when the query fails.
pub async fn http_urls(
    db: &PgPool,
    devices: &[DeviceId],
) -> Result<HashMap<DeviceId, Vec<(CheckId, String)>>, Error> {
    Ok(store::http_urls(db, devices).await?)
}

/// # Errors
/// `NotFound` when there is no such check.
pub async fn get(db: &PgPool, id: CheckId) -> Result<Check, Error> {
    store::fetch(db, id).await
}

/// In one query (API-005); an id with no check is absent.
///
/// # Errors
/// `Unreadable` when a stored configuration cannot be read.
pub async fn get_many(db: &PgPool, ids: &[CheckId]) -> Result<Vec<Check>, Error> {
    store::fetch_many(db, ids).await
}

/// # Errors
/// `Database` when the query fails.
pub async fn resolved_ips(
    db: &PgPool,
    devices: &[DeviceId],
) -> Result<HashMap<DeviceId, IpAddr>, Error> {
    store::resolved_ips(db, devices).await
}

/// D-26, DB-011.
///
/// # Errors
/// `Database` when the read fails.
pub async fn defaults(db: &PgPool) -> Result<CheckDefaults, Error> {
    store::read_defaults(db).await
}

pub(in crate::fleet::checks) async fn set_defaults(
    conn: &mut PgConnection,
    new: CheckDefaults,
) -> Result<CheckDefaults, Error> {
    store::write_defaults(conn, new).await
}

/// Spec 6.2, D-26.
///
/// # Errors
/// `Validation` (`deviceId`) when there is no such device.
pub async fn add_default_check(
    conn: &mut PgConnection,
    device: DeviceId,
) -> Result<CheckId, Error> {
    let id = CheckId::from(Uuid::now_v7());
    let settings = store::read_defaults(&mut *conn).await?.settings();
    let config = CheckConfig::Icmp {
        timeout_ms: 2000,
        ip_version: IpVersion::V4,
    };
    let row = store::Insert {
        device,
        parent: None,
        name: "Ping",
        config: &config,
        settings,
        push_token: None,
        sealed: None,
    };
    Ok(store::insert(conn, id, row).await?.id)
}

pub(in crate::fleet::checks) fn fresh_push_token() -> Result<PushToken, Error> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).map_err(Error::Random)?;
    Ok(PushToken::from_random(&bytes))
}

/// A parent must be a group on the same device. `sealer.host` is the host of `new.device`; SNMP
/// secrets are bound to it (SEC-010).
///
/// # Errors
/// `Validation` with every field in error; `Credentials` when sealing fails.
pub async fn create_check(
    conn: &mut PgConnection,
    sealer: Sealer<'_>,
    new: NewCheck,
) -> Result<Check, Error> {
    let id = CheckId::from(Uuid::now_v7());
    let prepared = rules::prepare(new.definition, rules::Stored::Nothing);
    let (name, prepared) = match (rules::name(&new.name), prepared) {
        (Ok(name), Ok(prepared)) => (name, prepared),
        (name, prepared) => {
            let errors = [name.err(), prepared.err()].into_iter().flatten().collect();
            return Err(Error::Validation(errors));
        }
    };
    let row = store::Insert {
        device: new.device,
        parent: new.parent,
        name: &name,
        config: &prepared.config,
        settings: new.settings,
        push_token: prepared.push_token.as_ref(),
        sealed: sealed(sealer, id, &prepared)?,
    };
    store::insert(conn, id, row).await
}

/// The type of a check never changes. `sealer.host` is the host of the check's device; SNMP
/// secrets are kept and sealed only for it (SEC-010).
///
/// # Errors
/// `NotFound`; `Validation` with the fields in error; `Credentials` when sealing fails.
pub async fn update_check(
    conn: &mut PgConnection,
    sealer: Sealer<'_>,
    id: CheckId,
    mut change: CheckChange,
) -> Result<Check, Error> {
    let definition = change.definition.take();
    let name = change.name.as_deref().map(rules::name).transpose();
    let mut tx = conn.begin().await?;
    let current = store::lock(&mut tx, id).await?;
    let prepared = match definition {
        Some(definition) => prepare_change(sealer, id, current, definition).map(Some),
        None => Ok(None),
    };
    let (name, prepared) = match (name, prepared) {
        (Ok(name), Ok(prepared)) => (name, prepared),
        (name, prepared) => {
            let errors = [name.err(), prepared.err()].into_iter().flatten().collect();
            return Err(Error::Validation(errors));
        }
    };
    let config = prepared
        .as_ref()
        .map(|prepared| Ok::<_, Error>((&prepared.config, sealed(sealer, id, prepared)?)))
        .transpose()?;
    let changed = store::Change {
        fields: &change,
        name: name.as_deref(),
        config,
        push_token: prepared.as_ref().and_then(|p| p.push_token.as_ref()),
    };
    let check = store::update(&mut tx, id, changed).await?;
    tx.commit().await?;
    Ok(check)
}

/// Its children leave the group.
///
/// # Errors
/// `NotFound` when there is no such check.
pub async fn delete_check(conn: &mut PgConnection, id: CheckId) -> Result<Check, Error> {
    store::delete(conn, id).await
}

/// Either way the check's state starts over (CHK-005).
///
/// # Errors
/// `NotFound` when there is no such check.
pub async fn set_paused(
    conn: &mut PgConnection,
    id: CheckId,
    paused: bool,
) -> Result<Check, Error> {
    store::set_paused(conn, id, paused).await
}

/// For the check's probe only: the secrets go only where they were given for (SEC-005).
///
/// # Errors
/// `Credentials`, `SecretsUnbound` or `Unreadable` when the secrets cannot be used.
pub async fn load_http_secrets(
    db: &PgPool,
    vault: &Vault,
    check: CheckId,
) -> Result<Option<HttpSetup>, Error> {
    store::load_http_secrets(db, vault, check).await
}

/// For a probe of `host` only: pass the host the probe sends to (SEC-005, SEC-010).
///
/// # Errors
/// `Credentials`, `SecretsUnbound`, `SecretsForOtherHost` or `Unreadable`.
pub async fn load_snmp_secrets(
    db: &PgPool,
    vault: &Vault,
    check: CheckId,
    host: &str,
) -> Result<Option<SnmpSetup>, Error> {
    store::load_snmp_secrets(db, vault, check, host).await
}

/// A paused check, or a beat not newer than the check's last beat, writes nothing and gives
/// [`Recorded::Skipped`] (CHK-003, CHK-005, DB-008).
///
/// # Errors
/// `NotFound` when the check no longer exists; nothing is written then.
pub async fn record_beat(
    conn: &mut PgConnection,
    check: CheckId,
    beat: &Beat,
) -> Result<Recorded, Error> {
    store::record_beat(conn, check, beat).await
}

/// Evaluates under the check's lock, so a beat, pause or change that landed meanwhile applies
/// (CHK-002, CHK-005).
pub(in crate::fleet::checks) async fn record_attempt(
    conn: &mut PgConnection,
    check: CheckId,
    attempt: &Attempt<'_>,
) -> Result<Option<Beat>, Error> {
    store::record_attempt(conn, check, attempt).await
}

/// PUSH checks are included: when one is due, its push did not come in its interval (CHK-001).
///
/// # Errors
/// `Database` when the query fails.
pub async fn due_checks(db: &PgPool, now: DateTime<Utc>, limit: u32) -> Result<Vec<Due>, Error> {
    store::due(db, now, limit).await
}

/// The daily retention run of checks (D-25, HUB-005, DB-004, DB-005, DB-009, DEV-015). The
/// database refuses to drop a heartbeat partition less than 1 day old.
///
/// # Errors
/// `Database` when a step fails, or when the database refuses the cutoff.
pub async fn drop_expired(
    db: &PgPool,
    now: DateTime<Utc>,
    heartbeats_keep: Days,
) -> Result<(), Error> {
    store::create_days_ahead(db, now, PARTITION_DAYS_AHEAD).await?;
    let dropped = store::drop_heartbeat_days_before(db, now, heartbeats_keep).await?;
    if !dropped.is_empty() {
        tracing::info!(domain = "checks", dropped = ?dropped, "old heartbeat partitions dropped");
    }
    store::trim_aggregates(db, now).await?;
    store::delete_orphan_heartbeats(db).await?;
    Ok(())
}
