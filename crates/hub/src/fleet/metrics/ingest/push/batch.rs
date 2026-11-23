//! One accepted batch written in one transaction (K8 Section 4; MET-006, MET-015, MET-018,
//! MET-032, CAP-029; R-098 a, b, e; R-122; R-135).
//! The device row is taken first, the order a device delete and a revoke use, so none deadlock.

use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::{
    agent_usage::AgentUsage,
    hardware::Hardware,
    inventory::Inventory,
    readings::Drive,
    wire::{RefusalCode, ThreadSummaries, ThreadSummariesReceipt, TokenHash},
};
use sqlx::{PgConnection, PgPool};

use super::samples::Rows;
use crate::{
    fleet::{
        agent_threads, agent_usage,
        devices::{self, MetricsSource},
        events::{self, ActorKind, Bus, NewEvent, Severity, Source},
        metrics::{
            Battery, Gpu,
            ingest::{Error, need_gone, store},
            store::{self as metrics_store, Discovery},
        },
    },
    ids::DeviceId,
};

const NEW_SERIES_SPAN: TimeDelta = TimeDelta::hours(1);

/// Per device per hour (R-098 e, CR-20), counted by the hub's creation time so point times cannot reset it.
/// One daemon has at most 623 series; the rest is room for parts that come and go within the hour.
pub(super) const MAX_NEW_SERIES: i64 = 1024;

pub(super) struct Push<'a> {
    pub(super) device: DeviceId,
    pub(super) name: &'a str,
    pub(super) token: TokenHash,
    /// `None`: the GPU set is unchanged; `Some(&[])`: the device has no GPUs now (R-098 b).
    pub(super) gpus: Option<&'a [Gpu]>,
    /// `None`: the drives are unchanged; `Some(&[])`: the device has no drives now (MET-027).
    pub(super) drives: Option<&'a [Drive]>,
    /// `None`: the batteries are unchanged; `Some(&[])`: the device has no battery now (MET-032).
    pub(super) batteries: Option<&'a [Battery]>,
    /// `None`: the tool inventory is unchanged; `Some`: it replaces the stored one (CAP-029).
    pub(super) inventory: Option<&'a Inventory>,
    /// `None`: the hardware values are unchanged; `Some`: they replace the stored ones (CAP-032).
    pub(super) hardware: Option<&'a Hardware>,
    /// The version of the daemon that sent the batch; `None` from a daemon older than R-122.
    pub(super) daemon_version: Option<&'a str>,
    /// `None`: nothing new; `Some`: its state and session days are stored (R-135, AIS-020).
    pub(super) agent_usage: Option<&'a AgentUsage>,
    pub(super) agent_threads: Option<&'a ThreadSummaries>,
}

pub(super) enum Written {
    /// Stored, with how many of its samples were new.
    Kept(i64, Option<ThreadSummariesReceipt>),
    /// The source or token changed since the check (`TOKEN_REFUSED`), or it would pass [`MAX_NEW_SERIES`] (`MALFORMED`).
    Refused(RefusalCode),
}

pub(super) async fn write(
    db: &PgPool,
    bus: &Bus,
    (push, rows): (Push<'_>, &Rows),
    now: DateTime<Utc>,
) -> Result<Written, Error> {
    let mut tx = db.begin().await?;
    // Checked again under the row lock: a switch away from the daemon that committed meanwhile refuses the batch.
    match devices::lock_metrics_source(&mut tx, push.device).await {
        Ok(MetricsSource::Daemon) => {}
        Ok(MetricsSource::None | MetricsSource::Ssh) | Err(devices::Error::NotFound(_)) => {
            return Ok(Written::Refused(RefusalCode::TokenRefused));
        }
        Err(other) => return Err(Error::Devices(other)),
    }
    let Some(taken) = store::take_batch(&mut tx, push.device, push.token, now).await? else {
        return Ok(Written::Refused(RefusalCode::TokenRefused));
    };
    store::record_version(&mut tx, push.device, push.daemon_version, now).await?;
    let since = now - NEW_SERIES_SPAN;
    let created = metrics_store::created_series(&mut tx, push.device, since, &rows.series).await?;
    if created > MAX_NEW_SERIES {
        return Ok(Written::Refused(RefusalCode::Malformed));
    }
    if write_announcements(&mut tx, &push, now).await? {
        events::record(
            &mut tx,
            bus,
            now,
            NewEvent {
                severity: Severity::Info,
                source: Source::Metrics,
                device: Some((push.device, push.name.to_owned())),
                message: format!("Private remote access configured for {}.", push.name),
                actor: ActorKind::Hub,
            },
        )
        .await?;
    }
    metrics_store::add_series(&mut tx, push.device, &rows.series, now).await?;
    let kept = metrics_store::add_points(&mut tx, push.device, &rows.values).await?;
    if taken.was_stale {
        events::record(&mut tx, bus, now, fresh_again(push.device, push.name)).await?;
    }
    if taken.needed {
        events::record(&mut tx, bus, now, need_gone(push.device, push.name)).await?;
    }
    let receipt = agent_threads::record_announced(&mut tx, push.device, push.agent_threads).await?;
    tx.commit().await?;
    Ok(Written::Kept(kept, receipt))
}

async fn write_announcements(
    tx: &mut PgConnection,
    push: &Push<'_>,
    now: DateTime<Utc>,
) -> Result<bool, Error> {
    let mut changed = false;
    if let Some(gpus) = push.gpus {
        // No boot time: the daemon's own discovery cadence replaces the collector's boot check (MET-006).
        let discovery = Discovery {
            found_at: now,
            boot_at: None,
            gpus,
        };
        metrics_store::write_gpus(&mut *tx, push.device, discovery).await?;
    }
    if let Some(drives) = push.drives {
        metrics_store::write_drives(&mut *tx, push.device, drives, now).await?;
    }
    if let Some(batteries) = push.batteries {
        metrics_store::write_batteries(&mut *tx, push.device, batteries, now).await?;
    }
    if let Some(inventory) = push.inventory {
        metrics_store::write_inventory(&mut *tx, push.device, inventory, now).await?;
        if let Some(access) = &inventory.remote_access {
            changed = devices::configure_private_access(&mut *tx, push.device, access).await?;
        }
    }
    if let Some(hardware) = push.hardware {
        metrics_store::write_hardware(&mut *tx, push.device, hardware, now).await?;
    }
    if let Some(usage) = push.agent_usage {
        agent_usage::record(&mut *tx, push.device, usage, now).await?;
    }
    Ok(changed)
}

/// The one event of a daemon that sends again after it was marked stale (MET-018).
fn fresh_again(device: DeviceId, name: &str) -> NewEvent {
    NewEvent {
        severity: Severity::Info,
        source: Source::Metrics,
        device: Some((device, name.to_owned())),
        message: format!("The daemon on {name} sends metrics again."),
        actor: ActorKind::Hub,
    }
}
