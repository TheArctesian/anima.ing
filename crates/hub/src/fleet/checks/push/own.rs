//! Checks the hub reports itself (HMD-045, HMD-047): PUSH checks found by name, so a hub that
//! stops reporting gets a missed push.

use std::time::Duration;

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{Report, report_in};
use crate::{
    fleet::{
        checks::{Error, config::CheckConfig, fresh_push_token, store},
        devices,
        events::Bus,
    },
    ids::{CheckId, DeviceId},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnCheck {
    /// The hub runs (HMD-045).
    HubRunning,
    /// The computer is connected to the link of the hub (HMD-047).
    Connected,
}

impl OwnCheck {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::HubRunning => "Hub running",
            Self::Connected => "Connected",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnReport {
    Up,
    /// Down, with the reason that the check's history shows.
    Down(&'static str),
}

/// The PUSH check `own` of `device`, made in the caller's transaction when missing.
///
/// # Errors
/// `Validation` (`deviceId`) when there is no such device; `Random` when no token can be made.
pub async fn ensure(
    conn: &mut PgConnection,
    device: DeviceId,
    own: OwnCheck,
) -> Result<CheckId, Error> {
    if let Some(id) = store::push_check_named(&mut *conn, device, own.name()).await? {
        return Ok(id);
    }
    let settings = store::read_defaults(&mut *conn).await?.settings();
    let token = fresh_push_token()?;
    let row = store::Insert {
        device,
        parent: None,
        name: own.name(),
        config: &CheckConfig::Push,
        settings,
        push_token: Some(&token),
        sealed: None,
    };
    let id = CheckId::from(Uuid::now_v7());
    Ok(store::insert(conn, id, row).await?.id)
}

/// The PUSH check `own` of `device`, when the device has it. Nothing is added.
///
/// # Errors
/// `Database` when the read fails.
pub async fn find(db: &PgPool, device: DeviceId, own: OwnCheck) -> Result<Option<CheckId>, Error> {
    let mut conn = db.acquire().await?;
    store::push_check_named(&mut conn, device, own.name()).await
}

/// Reports as a push of the device does and returns the wait for the next report; `None` when
/// the check or its device is gone. A Down report is one failed attempt (CHK-002).
///
/// # Errors
/// `Database`, `Devices` or `Maintenance` when a read or the write fails.
pub async fn report(
    (db, bus): (&PgPool, &Bus),
    check: CheckId,
    report: OwnReport,
    now: DateTime<Utc>,
) -> Result<Option<Duration>, Error> {
    let check = match store::fetch(db, check).await {
        Ok(check) => check,
        Err(Error::NotFound(_)) => return Ok(None),
        Err(error) => return Err(error),
    };
    let report = Report {
        up: report == OwnReport::Up,
        message: match report {
            OwnReport::Up => None,
            OwnReport::Down(reason) => Some(reason.to_owned()),
        },
        ping_ms: None,
    };
    match report_in((db, bus), &check, report, now).await {
        Ok(()) => {}
        Err(Error::NotFound(_) | Error::Devices(devices::Error::NotFound(_))) => return Ok(None),
        Err(error) => return Err(error),
    }
    let interval = u64::from(check.settings.interval_seconds);
    Ok(Some(Duration::from_secs(interval)))
}

/// Postpones each check `own` to one interval from `now`, so time the hub did not run is not
/// a missed push.
///
/// # Errors
/// `Database` when the write fails.
pub async fn await_reports(db: &PgPool, own: OwnCheck, now: DateTime<Utc>) -> Result<(), Error> {
    store::postpone_push_named(db, own.name(), now).await
}
