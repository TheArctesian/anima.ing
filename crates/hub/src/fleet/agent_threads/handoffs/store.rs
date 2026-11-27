use super::{Error, Offer, Result, Transfer, activation, placement, withdrawal};
use crate::ids::DeviceId;
use agent_threads::managed::workspace::handoff::{Import, Receipt};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::Value;
use sqlx::{PgConnection, PgPool, types::Json};
use uuid::Uuid;

pub(super) async fn find(tx: &mut PgConnection, transfer: Uuid) -> Result<Option<Transfer>> {
    Ok(sqlx::query_as(
        "SELECT * FROM agent_threads.workspace_handoffs WHERE transfer=$1 FOR UPDATE",
    )
    .bind(transfer)
    .fetch_optional(tx)
    .await?)
}

pub(super) async fn status(
    db: &PgPool,
    transfer: Uuid,
    principal: Uuid,
    now: DateTime<Utc>,
) -> Result<Option<Value>> {
    Ok(sqlx::query_scalar(
        "SELECT jsonb_build_object('transfer',transfer,'workspace',workspace,
        'sourceDevice',source_device,'destinationDevice',destination_device,'generation',generation,
        'expiresAt',expires_at,'sourceRetired',retired_at IS NOT NULL,'state',CASE
            WHEN finished_at IS NOT NULL AND withdraw_requested_at IS NOT NULL THEN 'withdrawn'
            WHEN finished_at IS NOT NULL THEN 'activated'
            WHEN withdraw_requested_at IS NOT NULL THEN 'withdrawing'
            WHEN expires_at <= $3 AND activation_offered_at IS NOT NULL THEN 'checking-activation'
            WHEN expires_at <= $3 THEN 'expired'
            WHEN activation_offered_at IS NOT NULL THEN 'activating'
            WHEN restored_at IS NOT NULL THEN 'restored' WHEN confirmed THEN 'ready'
            WHEN checkpoint IS NOT NULL THEN 'staged' ELSE 'awaiting-export' END)
        FROM agent_threads.workspace_handoffs WHERE transfer=$1 AND principal=$2",
    )
    .bind(transfer)
    .bind(principal)
    .bind(now)
    .fetch_optional(db)
    .await?)
}

pub(crate) async fn issued(
    tx: &mut PgConnection,
    device: DeviceId,
    now: DateTime<Utc>,
) -> Result<Option<Offer>> {
    let transfer: Option<Transfer> = sqlx::query_as("SELECT * FROM agent_threads.workspace_handoffs
        WHERE CASE WHEN retire_requested_at IS NOT NULL THEN
          source_device=$1 AND retired_at IS NULL
        WHEN withdraw_requested_at IS NOT NULL THEN
          (destination_device=$1 AND confirmed AND destination_discarded_at IS NULL)
          OR (source_device=$1 AND finished_at IS NULL
             AND (activation_offered_at IS NULL OR destination_discarded_at IS NOT NULL))
        ELSE finished_at IS NULL AND (
          (expires_at>$2 AND ((source_device=$1 AND NOT confirmed) OR (destination_device=$1 AND confirmed)))
          OR (destination_device=$1 AND activation_offered_at IS NOT NULL)) END
        ORDER BY (retire_requested_at IS NULL) DESC,(withdraw_requested_at IS NOT NULL) DESC,(expires_at>$2) DESC,created_at,transfer LIMIT 1 FOR UPDATE")
        .bind(device).bind(now).fetch_optional(&mut *tx).await?;
    let Some(transfer) = transfer else {
        return placement::issue(tx, device, now).await;
    };
    if transfer.retire_requested_at.is_some() {
        return Ok(Some(Offer::Retire {
            source: transfer.source().ok_or_else(refused)?,
            expires_at: now + TimeDelta::seconds(90),
        }));
    }
    if transfer.withdraw_requested_at.is_some() {
        return withdrawal::offer(&transfer, device, now).map(Some);
    }
    if transfer.source_device == device {
        return export_offer(transfer).map(Some);
    }
    destination_offer(tx, &transfer, now).await.map(Some)
}

pub(super) fn export_offer(transfer: Transfer) -> Result<Offer> {
    Ok(Offer::Export {
        source: transfer.source().ok_or_else(refused)?,
        expires_at: transfer.expires_at,
        staged: transfer.checkpoint.map(|receipt| receipt.0),
    })
}

async fn destination_offer(
    tx: &mut PgConnection,
    transfer: &Transfer,
    now: DateTime<Utc>,
) -> Result<Offer> {
    let import = transfer.checkpoint_import().ok_or_else(refused)?;
    let expires_at = transfer.expires_at;
    if now >= expires_at {
        return Ok(Offer::InspectActivation {
            import,
            expires_at: now + TimeDelta::seconds(15),
        });
    }
    if transfer.restored_at.is_none() {
        return Ok(Offer::Restore { import, expires_at });
    }
    activation::authorize(tx, transfer, now).await?;
    Ok(Offer::Activate { import, expires_at })
}

pub(crate) async fn prepare(
    tx: &mut PgConnection,
    device: DeviceId,
    receipt: &Receipt,
    now: DateTime<Utc>,
) -> Result<Transfer> {
    let transfer = find(tx, receipt.source.binding.transfer)
        .await?
        .filter(|transfer| transfer.live(now) && transfer.source_device == device)
        .ok_or_else(refused)?;
    if transfer.import(receipt).is_none()
        || transfer
            .checkpoint
            .as_ref()
            .is_some_and(|held| &held.0 != receipt)
    {
        return Err(refused());
    }
    Ok(transfer)
}

pub(crate) async fn staged(tx: &mut PgConnection, receipt: &Receipt) -> Result<()> {
    sqlx::query("UPDATE agent_threads.workspace_handoffs SET checkpoint=$2 WHERE transfer=$1 AND checkpoint IS NULL")
        .bind(receipt.source.binding.transfer).bind(Json(receipt)).execute(tx).await?;
    Ok(())
}

pub(crate) async fn confirmed(tx: &mut PgConnection, receipt: &Receipt) -> Result<()> {
    sqlx::query("UPDATE agent_threads.workspace_handoffs SET confirmed=true WHERE transfer=$1")
        .bind(receipt.source.binding.transfer)
        .execute(tx)
        .await?;
    Ok(())
}

pub(crate) async fn read(
    tx: &mut PgConnection,
    device: DeviceId,
    id: Uuid,
    now: DateTime<Utc>,
) -> Result<Transfer> {
    find(tx, id)
        .await?
        .filter(|transfer| {
            transfer.destination_device == device
                && transfer.confirmed
                && transfer.restored_at.is_none()
                && transfer.live(now)
        })
        .ok_or_else(refused)
}

fn refused() -> Error {
    Error::Refused("This device does not hold the workspace transfer.")
}

pub(crate) async fn restored(
    tx: &mut PgConnection,
    device: DeviceId,
    import: &Import,
    now: DateTime<Utc>,
) -> Result<()> {
    let id = import.receipt.source.binding.transfer;
    let transfer = find(tx, id)
        .await?
        .filter(|transfer| {
            transfer.destination_device == device
                && transfer.confirmed
                && (transfer.live(now) || transfer.restored_at.is_some())
        })
        .ok_or_else(refused)?;
    let expected = transfer.checkpoint_import();
    if expected.as_ref() != Some(import) {
        return Err(refused());
    }
    sqlx::query(
        "UPDATE agent_threads.workspace_handoffs SET restored_at=$2
        WHERE transfer=$1 AND restored_at IS NULL",
    )
    .bind(id)
    .bind(now)
    .execute(tx)
    .await?;
    Ok(())
}
