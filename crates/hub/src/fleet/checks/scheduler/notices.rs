//! LOG-004, LOG-010, CHK-010.

use std::collections::HashMap;

use chrono::{DateTime, Utc};

use super::{
    attempt::{Context, warn},
    log::{logged, warned},
    schedule,
};
use crate::{
    fleet::{
        checks::{self, Due, Missing},
        devices,
        events::{self, ActorKind, NewEvent, Severity, Signal, Source},
    },
    ids::DeviceId,
    status::Status,
};

/// A change is published only once stored, so a failed write leaves the device unsettled
/// for the next tick (LOG-004).
pub(super) async fn settle(
    ctx: &Context,
    known: &mut HashMap<DeviceId, Status>,
    device: DeviceId,
    now: DateTime<Utc>,
) -> bool {
    let status = match checks::device_status(&ctx.db, device, now).await {
        Ok(status) => status,
        Err(checks::Error::NotFound(Missing::Device(_))) => return forget(known, device),
        Err(error) => return unsettled(device, "the device status cannot be read", &error),
    };
    if known.get(&device) == Some(&status) {
        return true;
    }
    let name = match devices::get(&ctx.db, device).await {
        Ok(found) => found.name,
        Err(devices::Error::NotFound(_)) => return forget(known, device),
        Err(error) => return unsettled(device, "the device cannot be read", &error),
    };
    let event = NewEvent {
        severity: schedule::severity_of(status),
        source: Source::Check,
        device: Some((device, name.clone())),
        message: schedule::status_message(&name, status),
        actor: ActorKind::Hub,
    };
    let written = write_event(ctx, event, now).await;
    if logged(
        Some(device),
        "the status change event cannot be written",
        written,
    )
    .is_none()
    {
        return false;
    }
    ctx.bus.publish(Signal::DeviceStatus {
        device,
        status,
        at: now,
    });
    known.insert(device, status);
    true
}

fn unsettled(device: DeviceId, what: &'static str, error: &dyn std::fmt::Display) -> bool {
    warned(Some(device), what, error);
    false
}

fn forget(known: &mut HashMap<DeviceId, Status>, device: DeviceId) -> bool {
    known.remove(&device);
    true
}

pub(super) async fn report_late(ctx: &Context, check: &Due, now: DateTime<Utc>) -> bool {
    let names = async {
        let name = checks::get(&ctx.db, check.id).await?.name;
        let device = devices::get(&ctx.db, check.device)
            .await
            .map_err(checks::from_devices)?
            .name;
        Ok::<_, checks::Error>((name, device))
    };
    let (name, device) = match names.await {
        Ok(names) => names,
        Err(error) => {
            warn(check, "the late check cannot be read", &error);
            return false;
        }
    };
    let event = NewEvent {
        severity: Severity::Error,
        source: Source::Check,
        device: Some((check.device, device.clone())),
        message: schedule::late_message(&name, &device),
        actor: ActorKind::Hub,
    };
    match write_event(ctx, event, now).await {
        Ok(()) => true,
        Err(error) => {
            warn(check, "the late check event cannot be written", &error);
            false
        }
    }
}

async fn write_event(
    ctx: &Context,
    event: NewEvent,
    now: DateTime<Utc>,
) -> Result<(), events::Error> {
    let mut tx = ctx.db.begin().await?;
    events::record(&mut tx, &ctx.bus, now, event).await?;
    tx.commit().await?;
    Ok(())
}
