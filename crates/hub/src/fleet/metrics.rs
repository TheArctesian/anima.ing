//! Resource metrics of a device (spec 6.6, HUB-004, MET-001, MET-002, MET-006).
//!
//! The device is not trusted, so no reader panics and none fails on a bad line.

mod capabilities;
mod collector;
mod commands;
mod feed;
mod gpu_label;
pub mod graphql;
mod ingest;
mod latest;
mod model;
mod range;
mod retention;
mod storage;
mod store;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Days, TimeDelta, Utc};
use sqlx::PgPool;

pub use capabilities::{Capabilities, CapabilityFilter, DeviceCapabilities};
pub use collector::{Collecting, spawn_collector};
pub use daemon_core::{
    points::{MAX_LABELS, Sample, samples},
    readings::{Cpu, Disk, Interface, Load, Memory, Reading},
};
pub use feed::Feed;
pub use ingest::{
    DaemonEnrolment, Error as DaemonError, daemon_router, enrol_code, spawn_stale_watch,
};
pub use model::{
    Battery, BatteryState, DaemonNeed, Gpu, GpuVendor, MetricKind, MetricPoint, MetricRange,
    MetricSample, MetricSeries, MetricsAdded, Readings,
};
pub use retention::{Retention, Trimmed};
pub use storage::{DriveKind, Mount, Storage, StorageDrive};

use crate::ids::DeviceId;

/// How long a sample counts as a device's current reading: four collections (D-8).
const READING_WINDOW: TimeDelta = TimeDelta::minutes(2);

/// How many days of sample partitions exist ahead of today (DB-004, D-6).
const DAYS_AHEAD: i32 = 7;

/// Why a metrics operation failed. Messages hold no SQL and no input value.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("the device does not exist")]
    NoDevice,
    #[error("a stored metric row cannot be read")]
    Unreadable,
}

/// Writes one collection with its series and aggregates together (DB-008); a sample already
/// stored for its series and time is left as it is.
///
/// # Errors
/// `NoDevice` when the device does not exist; `Database` when a query fails or `at` is outside
/// the window the database makes partitions for.
pub async fn record_collection(
    db: &PgPool,
    device: DeviceId,
    at: DateTime<Utc>,
    samples: &[Sample],
) -> Result<(), Error> {
    if samples.is_empty() {
        return Ok(());
    }
    store::record(db, device, at, samples).await
}

/// The current readings of each of `devices` with a sample in [`READING_WINDOW`] before `now`
/// (MET-004, MET-030, D-8); a device without one has no entry.
///
/// # Errors
/// `Database` when a query fails; `Unreadable` for a stored kind this version does not know.
pub async fn readings(
    db: &PgPool,
    devices: &[DeviceId],
    now: DateTime<Utc>,
) -> Result<HashMap<DeviceId, Readings>, Error> {
    let latest = store::latest(db, (devices, &latest::READ_KINDS), now, READING_WINDOW).await?;
    let mut readings = latest::readings(latest);
    let held: Vec<DeviceId> = readings.keys().copied().collect();
    latest::count_other_drives(&mut readings, &store::drives(db, &held).await?);
    Ok(readings)
}

/// The storage of `device` drive by drive (MET-026, MET-029, D-8); a device whose drives the hub
/// does not know lists its mounts.
///
/// # Errors
/// `Database` when a query fails; `Unreadable` for a stored kind or drive kind this version
/// does not know.
pub async fn storage(db: &PgPool, device: DeviceId, now: DateTime<Utc>) -> Result<Storage, Error> {
    let devices = [device];
    let latest =
        store::latest(db, (&devices, &storage::STORAGE_KINDS), now, READING_WINDOW).await?;
    let drives = store::drives(db, &devices).await?.remove(&device);
    Ok(storage::storage(&drives.unwrap_or_default(), &latest))
}

/// Points are oldest first; a series without a point in the range is left out (MET-003, D-7).
///
/// # Errors
/// `Database` when the query fails; `Unreadable` for a stored kind this version does not know.
pub async fn series(
    db: &PgPool,
    device: DeviceId,
    range: MetricRange,
    now: DateTime<Utc>,
) -> Result<Vec<MetricSeries>, Error> {
    store::chart(db, device, range::window(range, now)).await
}

/// A device with no GPU has no entry (MET-006).
///
/// # Errors
/// `Database` when the query fails; `Unreadable` for a stored vendor this version does not know.
pub async fn gpus(db: &PgPool, devices: &[DeviceId]) -> Result<HashMap<DeviceId, Vec<Gpu>>, Error> {
    store::gpus(db, devices).await
}

/// A device with no battery has no entry (MET-032).
///
/// # Errors
/// `Database` when the query fails; `Unreadable` for a stored state this version does not know.
pub async fn batteries(
    db: &PgPool,
    devices: &[DeviceId],
) -> Result<HashMap<DeviceId, Vec<Battery>>, Error> {
    store::batteries(db, devices).await
}

/// The tool inventories that `filter` admits, in no set order (CAP-030, CAP-032).
///
/// # Errors
/// `Database` when the query fails; `Unreadable` for a stored architecture or system this
/// version does not know.
pub async fn capabilities(
    db: &PgPool,
    filter: &CapabilityFilter,
) -> Result<Vec<Capabilities>, Error> {
    let mut found = store::inventories(db).await?;
    found.retain(|stored| filter.admits(&stored.inventory));
    let mut hardware = store::hardware(db).await?;
    for stored in &mut found {
        stored.hardware = hardware.remove(&stored.device);
    }
    Ok(found)
}

/// The hardware values each daemon sent, with when the hub stored them (CAP-001 to CAP-005).
///
/// # Errors
/// `Database` when the query fails; `Unreadable` for a stored row this version does not read.
pub async fn hardware(
    db: &PgPool,
) -> Result<HashMap<DeviceId, (daemon_core::hardware::Hardware, DateTime<Utc>)>, Error> {
    store::hardware(db).await
}

/// Of `devices`, those whose daemon's metrics are stale (MET-018). The answer means something
/// only for a device whose metrics source is `daemon`.
///
/// # Errors
/// `Database` when the query fails.
pub async fn stale_daemons(db: &PgPool, devices: &[DeviceId]) -> Result<HashSet<DeviceId>, Error> {
    Ok(ingest::without_fresh_daemon(db, devices).await?)
}

/// The daemon version from each device's last batch; a device without one has no entry (R-122,
/// ROL-051).
///
/// # Errors
/// `Database` when the query fails.
pub async fn daemon_versions(
    db: &PgPool,
    devices: &[DeviceId],
) -> Result<HashMap<DeviceId, String>, Error> {
    Ok(ingest::daemon_versions(db, devices).await?)
}

/// What each daemon waits for from a person at the device; a device without a need has no entry
/// (R-364).
///
/// # Errors
/// `Database` when the query fails; `Unreadable` for a stored need this version does not know.
#[expect(
    clippy::zero_sized_map_values,
    reason = "DaemonNeed has one kind today; the map keeps the kind for the kinds that K1 adds"
)]
pub async fn daemon_needs(
    db: &PgPool,
    devices: &[DeviceId],
) -> Result<HashMap<DeviceId, DaemonNeed>, Error> {
    ingest::daemon_needs(db, devices)
        .await?
        .into_iter()
        .map(|(device, text)| {
            DaemonNeed::from_stored(&text)
                .map(|need| (device, need))
                .ok_or(Error::Unreadable)
        })
        .collect()
}

/// The distribution an `os-release` file names (DEV-016, HMD-046).
#[must_use]
pub fn distribution_of(os_release: &[u8]) -> Option<crate::fleet::devices::Distribution> {
    collector::os_release::distribution(os_release)
}

/// HMD-071.
#[derive(Debug, thiserror::Error)]
#[error("the enrolment code of the hub's own daemon cannot be made")]
pub struct OwnCodeError(#[source] ingest::Error);

/// A one-use enrolment code for the hosted hub's own daemon; earlier unused codes of the device
/// stop working (HMD-071, HM-T35).
///
/// # Errors
/// [`OwnCodeError`] when the random source or the database fails, or the device is gone.
pub async fn own_daemon_code(
    db: &PgPool,
    device: DeviceId,
    now: DateTime<Utc>,
) -> Result<daemon_core::wire::EnrolmentCode, OwnCodeError> {
    let bytes = ingest::random::<{ daemon_core::wire::CODE_BYTES }>().map_err(OwnCodeError)?;
    ingest::own_device_code(db, device, *bytes, now)
        .await
        .map_err(OwnCodeError)
}

/// Creates the sample partitions of today (UTC) and the next 7 days (DB-004).
///
/// # Errors
/// `Database` when a query fails or `now` is past the database clock's partition window.
pub async fn create_partitions_ahead(db: &PgPool, now: DateTime<Utc>) -> Result<(), Error> {
    store::create_days_ahead(db, retention::start_of_day(now), DAYS_AHEAD).await
}

/// The daily retention run (DB-005, DB-009, DEV-015, D-5). `now` is held to the database clock,
/// so a caller cannot ask for more to go than the database's own time allows (CQ-021).
///
/// # Errors
/// `Database` when a query fails, or when the database refuses to drop a partition still
/// inside the 1-day floor.
pub async fn drop_expired_partitions(
    db: &PgPool,
    now: DateTime<Utc>,
    retention: Retention,
) -> Result<Trimmed, Error> {
    let now = now.min(store::database_now(db).await?);
    let cutoffs = retention.cutoffs(now);
    let partitions = store::drop_days_before(db, cutoffs.samples, now).await?;
    let [minute, hour, day] = store::trim_aggregates(db, &cutoffs).await?;
    let idle_series = store::delete_idle_series(db, cutoffs.minute).await?;
    let orphan_samples = store::delete_orphan_samples(db).await?;
    Ok(Trimmed {
        partitions,
        minute,
        hour,
        day,
        orphan_samples,
        idle_series,
    })
}

impl Error {
    /// For the process log: what failed, as a fixed word, never the source text (LOG-007).
    #[must_use]
    pub const fn fault(&self) -> &'static str {
        match self {
            Self::Database(_) => "database",
            Self::NoDevice => "no_device",
            Self::Unreadable => "unreadable",
        }
    }
}

/// Samples are kept `samples_keep`; the aggregates stay at their fixed periods (DB-004, DB-005,
/// DB-009, D-5, D-25).
///
/// # Errors
/// What [`create_partitions_ahead`] or [`drop_expired_partitions`] returns.
pub async fn keep_partitions(
    db: &PgPool,
    now: DateTime<Utc>,
    samples_keep: Days,
) -> Result<(), Error> {
    create_partitions_ahead(db, now).await?;
    let samples = now - now.checked_sub_days(samples_keep).unwrap_or(now);
    let retention = Retention {
        samples,
        ..Retention::default()
    };
    drop_expired_partitions(db, now, retention).await?;
    Ok(())
}
