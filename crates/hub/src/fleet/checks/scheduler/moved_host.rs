//! SNMP secrets whose device host changed after they were entered are removed, never sent (SEC-010).

use chrono::{DateTime, Utc};

use super::{
    attempt::{Context, warn},
    log::Fault,
    schedule,
};
use crate::fleet::{
    checks::{Due, store},
    devices::Device,
    events::{self, ActorKind, NewEvent, Severity, Source},
};

/// A failed write is logged; the next attempt tries again, and sends nothing either way.
pub(super) async fn forget_secrets(
    ctx: &Context,
    (due, device): (&Due, &Device),
    host: &str,
    now: DateTime<Utc>,
) {
    if let Err(error) = forget(ctx, (due, device), host, now).await {
        warn(
            due,
            "the SNMP secrets of a moved device cannot be removed",
            &error,
        );
    }
}

async fn forget(
    ctx: &Context,
    (due, device): (&Due, &Device),
    host: &str,
    now: DateTime<Utc>,
) -> Result<(), Fault> {
    let mut tx = ctx.db.begin().await?;
    let vault = &ctx.probing.vault;
    let Some(check) = store::forget_snmp_secrets(&mut tx, vault, due.id, host).await? else {
        return Ok(());
    };
    let event = NewEvent {
        severity: Severity::Warning,
        source: Source::Check,
        device: Some((device.id, device.name.clone())),
        message: schedule::moved_secrets_message(&check, &device.name),
        // The hub's own scheduler found the moved host; no request caused it (LOG-013).
        actor: ActorKind::Hub,
    };
    events::record(&mut tx, &ctx.bus, now, event).await?;
    tx.commit().await?;
    Ok(())
}
