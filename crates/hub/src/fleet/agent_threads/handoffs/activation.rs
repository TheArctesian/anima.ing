use super::{Error, Result, Transfer, store};
use crate::ids::DeviceId;
use agent_threads::managed::workspace::{
    Action, Checkout, Command, Phase, Receipt, View, handoff::Import,
};
use chrono::{DateTime, Utc};
use sqlx::{PgConnection, types::Json};
use uuid::Uuid;

pub(super) async fn authorize(
    tx: &mut PgConnection,
    transfer: &Transfer,
    now: DateTime<Utc>,
) -> Result<()> {
    sqlx::query(
        "UPDATE agent_threads.workspace_handoffs SET activation_offered_at=$2
        WHERE transfer=$1 AND activation_offered_at IS NULL",
    )
    .bind(transfer.id)
    .bind(now)
    .execute(tx)
    .await?;
    Ok(())
}

pub(crate) async fn activated(
    tx: &mut PgConnection,
    device: DeviceId,
    import: &Import,
    now: DateTime<Utc>,
) -> Result<()> {
    let id = import.receipt.source.binding.transfer;
    let transfer = store::find(tx, id)
        .await?
        .filter(|transfer| {
            transfer.destination_device == device
                && transfer.activation_offered_at.is_some()
                && transfer.destination_discarded_at.is_none()
                && transfer.checkpoint_import().as_ref() == Some(import)
        })
        .ok_or(Error::Refused(
            "This device does not hold the workspace activation.",
        ))?;
    if transfer.finished_at.is_some() {
        return Ok(());
    }
    adopt(tx, device, import, now).await?;
    sqlx::query(
        "UPDATE agent_threads.workspace_owners SET deleted=true
        WHERE device_id=$1 AND workspace=$2 AND principal=$3",
    )
    .bind(transfer.source_device)
    .bind(transfer.workspace)
    .bind(transfer.principal)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE agent_threads.workspace_handoffs SET finished_at=$2, withdraw_requested_at=NULL WHERE transfer=$1")
        .bind(id)
        .bind(now)
        .execute(tx)
        .await?;
    Ok(())
}

async fn adopt(
    tx: &mut PgConnection,
    device: DeviceId,
    import: &Import,
    now: DateTime<Utc>,
) -> Result<()> {
    let (source, quota) = (import.receipt.source, import.storage_bytes);
    let adopted = sqlx::query(
        "INSERT INTO agent_threads.workspace_owners AS w(device_id,workspace,principal,created_at)
        VALUES ($1,$2,$3,$4)
        ON CONFLICT (device_id,workspace) DO UPDATE SET deleted=false, created_at=EXCLUDED.created_at
        WHERE w.deleted AND w.principal=EXCLUDED.principal",
    )
    .bind(device)
    .bind(source.workspace)
    .bind(source.principal)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    if adopted.rows_affected() != 1 {
        return Err(Error::Refused(
            "The destination already owns this workspace identity.",
        ));
    }
    let command = Command {
        request: Uuid::now_v7(),
        workspace: source.workspace,
        principal: source.principal,
        action: Action::Prepare {
            storage_bytes: quota,
            source: import.checkout.clone(),
        },
    };
    let view = View {
        workspace: source.workspace,
        generation: source.generation,
        phase: Phase::Prepared,
        storage_bytes: quota,
        checkout: import.checkout.clone().map(|source| Checkout {
            source,
            ready: true,
        }),
    };
    record_view(tx, device, (command, view), now).await
}

pub(super) async fn record_view(
    tx: &mut PgConnection,
    device: DeviceId,
    (command, view): (Command, View),
    now: DateTime<Utc>,
) -> Result<()> {
    let (workspace, principal) = (command.workspace, command.principal);
    let receipt = Receipt {
        command: command.clone(),
        result: Some(view),
        failure: None,
    };
    if !receipt.fits() {
        return Err(Error::Refused("The reported workspace state is invalid."));
    }
    sqlx::query(
        "INSERT INTO agent_threads.workspace_requests
        (request,device_id,workspace,principal,payload,state,receipt,created_at,expires_at)
        VALUES ($1,$2,$3,$4,$5,'completed',$6,$7,$7)",
    )
    .bind(command.request)
    .bind(device)
    .bind(workspace)
    .bind(principal)
    .bind(Json(command))
    .bind(Json(receipt))
    .bind(now)
    .execute(tx)
    .await?;
    Ok(())
}
