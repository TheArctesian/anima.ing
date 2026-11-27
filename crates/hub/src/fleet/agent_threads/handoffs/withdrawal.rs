use super::{Error, Result, Transfer, activation::record_view, store};
use crate::ids::DeviceId;
use agent_threads::managed::workspace::{
    Action, Command, Phase, Source as Checkout, View,
    handoff::{Offer, Withdrawal},
};
use chrono::{DateTime, TimeDelta, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

pub(super) async fn request(
    tx: &mut PgConnection,
    (principal, device): (Uuid, DeviceId),
    (transfer, workspace): (Uuid, Uuid),
    now: DateTime<Utc>,
) -> Result<bool> {
    let held = store::find(tx, transfer)
        .await?
        .filter(|held| {
            held.principal == principal
                && held.source_device == device
                && held.workspace == workspace
        })
        .ok_or_else(refused)?;
    if held.withdraw_requested_at.is_some() {
        return Ok(false);
    }
    if held.finished_at.is_some() {
        return Err(Error::Refused(
            "The destination has already completed this move.",
        ));
    }
    sqlx::query(
        "UPDATE agent_threads.workspace_handoffs SET withdraw_requested_at=$2 WHERE transfer=$1",
    )
    .bind(transfer)
    .bind(now)
    .execute(tx)
    .await?;
    Ok(true)
}

pub(super) fn offer(held: &Transfer, device: DeviceId, now: DateTime<Utc>) -> Result<Offer> {
    let expires_at = now + TimeDelta::seconds(90);
    if held.destination_device == device {
        return Ok(Offer::Discard {
            import: held.checkpoint_import().ok_or_else(refused)?,
            expires_at,
        });
    }
    Ok(Offer::Withdraw {
        source: held.source().ok_or_else(refused)?,
        expires_at,
    })
}

pub(crate) async fn report(
    tx: &mut PgConnection,
    device: DeviceId,
    result: &Withdrawal,
    now: DateTime<Utc>,
) -> Result<()> {
    let id = result.transfer();
    let held = store::find(tx, id)
        .await?
        .filter(|held| held.withdraw_requested_at.is_some())
        .ok_or_else(refused)?;
    let statement = match result {
        Withdrawal::Discarded { import }
            if held.destination_device == device
                && held.confirmed
                && held.checkpoint_import().as_ref() == Some(import) =>
        {
            "UPDATE agent_threads.workspace_handoffs SET destination_discarded_at=$2
             WHERE transfer=$1 AND destination_discarded_at IS NULL"
        }
        Withdrawal::Withdrawn { source, stopped }
            if held.source_device == device
                && held.source() == Some(*source)
                && (held.activation_offered_at.is_none()
                    || held.destination_discarded_at.is_some()) =>
        {
            if let Some(view) = stopped
                && held.finished_at.is_none()
            {
                record_stopped(tx, &held, view, now).await?;
            }
            "UPDATE agent_threads.workspace_handoffs SET finished_at=$2
             WHERE transfer=$1 AND finished_at IS NULL"
        }
        _ => return Err(refused()),
    };
    sqlx::query(statement)
        .bind(id)
        .bind(now)
        .execute(tx)
        .await?;
    Ok(())
}

async fn record_stopped(
    tx: &mut PgConnection,
    held: &Transfer,
    view: &View,
    now: DateTime<Utc>,
) -> Result<()> {
    let generation = u64::try_from(held.generation).map_err(|_| refused())?;
    let checkout = view.checkout.as_ref().map(|receipt| &receipt.source);
    let held_checkout: Option<&Checkout> = held.checkout.as_ref().map(|source| &source.0);
    if !matches!(view.phase, Phase::Stopped) || checkout != held_checkout {
        return Err(refused());
    }
    let command = Command {
        request: Uuid::now_v7(),
        workspace: held.workspace,
        principal: held.principal,
        action: Action::Stop { generation },
    };
    record_view(tx, held.source_device, (command, view.clone()), now).await
}

fn refused() -> Error {
    Error::Refused("This device does not hold the workspace withdrawal.")
}
