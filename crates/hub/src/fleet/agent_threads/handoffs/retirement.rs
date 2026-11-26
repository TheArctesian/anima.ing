//! An activated move gives the operator an exact source copy to remove.
use super::{Error, Result, store};
use crate::ids::DeviceId;
use agent_threads::managed::workspace::handoff::Source;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub(super) async fn request(
    tx: &mut PgConnection,
    (principal, destination): (Uuid, DeviceId),
    (transfer, workspace): (Uuid, Uuid),
    now: DateTime<Utc>,
) -> Result<bool> {
    let held = store::find(tx, transfer)
        .await?
        .filter(|held| {
            held.principal == principal
                && held.workspace == workspace
                && held.destination_device == destination
                && held.finished_at.is_some()
                && held.activation_offered_at.is_some()
                && held.withdraw_requested_at.is_none()
        })
        .ok_or(Error::Refused(
            "Only the owner of an activated move may remove its retained source.",
        ))?;
    if held.retire_requested_at.is_some() {
        return Ok(false);
    }
    sqlx::query(
        "UPDATE agent_threads.workspace_handoffs SET retire_requested_at=$2 WHERE transfer=$1",
    )
    .bind(transfer)
    .bind(now)
    .execute(tx)
    .await?;
    Ok(true)
}

pub(crate) async fn report(
    tx: &mut PgConnection,
    device: DeviceId,
    source: Source,
    now: DateTime<Utc>,
) -> Result<()> {
    let id = source.binding.transfer;
    let held = store::find(tx, id)
        .await?
        .filter(|held| {
            held.source_device == device
                && held.retire_requested_at.is_some()
                && held.source() == Some(source)
        })
        .ok_or(Error::Refused(
            "This device does not hold the retained source cleanup.",
        ))?;
    if held.retired_at.is_none() {
        sqlx::query("UPDATE agent_threads.workspace_handoffs SET retired_at=$2 WHERE transfer=$1")
            .bind(id)
            .bind(now)
            .execute(tx)
            .await?;
    }
    Ok(())
}

pub(super) async fn pending(
    db: &PgPool,
    destination: DeviceId,
    principal: Uuid,
) -> Result<Vec<Value>> {
    Ok(sqlx::query_scalar("SELECT jsonb_build_object(
         'transfer',h.transfer,'workspace',h.workspace,'sourceDevice',h.source_device,'sourceName',d.name,
         'generation',h.generation,'pending',h.retire_requested_at IS NOT NULL)
        FROM agent_threads.workspace_handoffs h JOIN devices.devices d ON d.id=h.source_device
        WHERE h.destination_device=$1 AND h.principal=$2 AND h.finished_at IS NOT NULL
            AND h.activation_offered_at IS NOT NULL AND h.withdraw_requested_at IS NULL AND h.retired_at IS NULL
        ORDER BY h.finished_at DESC,h.transfer DESC LIMIT 128")
        .bind(destination).bind(principal).fetch_all(db).await?)
}
