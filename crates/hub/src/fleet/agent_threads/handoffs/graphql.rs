use std::sync::Arc;

use async_graphql::{Context, ID, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{Error, MoveRequest, Start, placement, retirement, withdrawal};
use crate::{
    api_errors::{ErrorCode, coded, internal, pool, schema_dependency},
    config::Config,
    fleet::events::{AuditAction, AuditTarget, Severity, Source, Trail, TrailEntry},
    ids::DeviceId,
};

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Action {
    WithdrawMove,
    RetireSource {
        confirmed: bool,
    },
    Move {
        destination_device: DeviceId,
        generation: u64,
        storage_bytes: u64,
        #[serde(default)]
        continue_session: bool,
    },
    MoveAuto {
        generation: u64,
        storage_bytes: u64,
        #[serde(default)]
        continue_session: bool,
    },
}

pub(in crate::fleet::agent_threads) async fn queue(
    ctx: &Context<'_>,
    (principal, device): (Uuid, DeviceId),
    (transfer, workspace): (Uuid, Uuid),
    value: Value,
) -> Result<ID> {
    if schema_dependency::<Arc<Config>>(ctx)?
        .artifact_storage
        .is_none()
    {
        return Err(coded(
            "The Hub has no private checkpoint storage.",
            ErrorCode::Conflict,
        ));
    }
    let action = serde_json::from_value(value)
        .map_err(|_| coded("Invalid workspace transfer request.", ErrorCode::Validation))?;
    let (destination_device, generation, storage_bytes, continue_session) = match action {
        Action::Move {
            destination_device,
            generation,
            storage_bytes,
            continue_session,
        } => (
            Some(destination_device),
            generation,
            storage_bytes,
            continue_session,
        ),
        Action::MoveAuto {
            generation,
            storage_bytes,
            continue_session,
        } => (None, generation, storage_bytes, continue_session),
        Action::WithdrawMove => {
            return control(ctx, (principal, device), (transfer, workspace), false).await;
        }
        Action::RetireSource { confirmed: true } => {
            return control(ctx, (principal, device), (transfer, workspace), true).await;
        }
        Action::RetireSource { confirmed: false } => {
            return Err(coded(
                "Confirm deletion of the retained source copy.",
                ErrorCode::Validation,
            ));
        }
    };
    let request = MoveRequest {
        transfer,
        workspace,
        source_device: device,
        generation,
        storage_bytes,
        continue_session,
    };
    queue_move(ctx, principal, request, destination_device).await
}

async fn queue_move(
    ctx: &Context<'_>,
    principal: Uuid,
    request: MoveRequest,
    destination_device: Option<DeviceId>,
) -> Result<ID> {
    let trail = Trail::from_context(ctx)?;
    let mut tx = pool(ctx)?.begin().await.map_err(internal)?;
    let changed = if let Some(destination_device) = destination_device {
        let start = Start {
            request,
            destination_device,
            headroom: None,
        };
        placement::request_named(&mut tx, principal, start, trail.now).await
    } else {
        placement::lock_request(&mut tx, request)
            .await
            .map_err(failed)?;
        placement::queue(&mut tx, principal, (request, None), trail.now).await
    };
    if changed.map_err(failed)? {
        trail.write(&mut tx, TrailEntry {
            source: Source::Device,
            severity: Severity::Info,
            device: None,
            message: "Managed workspace transfer requested.".to_owned(),
            action: AuditAction::Update,
            target: AuditTarget::Device(request.source_device),
            change: json!({"workspaceHandoff":request.transfer,"workspace":request.workspace,"destinationDevice":destination_device}),
        }).await.map_err(internal)?;
    }
    tx.commit().await.map_err(internal)?;
    Ok(ID(request.transfer.to_string()))
}

async fn control(
    ctx: &Context<'_>,
    actor: (Uuid, DeviceId),
    transfer: (Uuid, Uuid),
    retire: bool,
) -> Result<ID> {
    let trail = Trail::from_context(ctx)?;
    let mut tx = pool(ctx)?.begin().await.map_err(internal)?;
    let (changed, message, operation) = if retire {
        (
            retirement::request(&mut tx, actor, transfer, trail.now).await,
            "Retained workspace source cleanup requested.",
            "retire-source",
        )
    } else {
        (
            withdraw(&mut tx, actor, transfer, trail.now).await,
            "Managed workspace withdrawal requested.",
            "withdraw-move",
        )
    };
    if changed.map_err(failed)? {
        trail.write(&mut tx, TrailEntry {
            source: Source::Device,
            severity: Severity::Info,
            device: None,
            message: message.to_owned(),
            action: AuditAction::Update,
            target: AuditTarget::Device(actor.1),
            change: json!({"workspaceHandoff":transfer.0,"workspace":transfer.1,"operation":operation}),
        }).await.map_err(internal)?;
    }
    tx.commit().await.map_err(internal)?;
    Ok(ID(transfer.0.to_string()))
}

async fn withdraw(
    tx: &mut sqlx::PgConnection,
    actor: (Uuid, DeviceId),
    transfer: (Uuid, Uuid),
    now: chrono::DateTime<chrono::Utc>,
) -> super::Result<bool> {
    placement::lock_identity(tx, transfer.0).await?;
    if let Some(changed) = placement::withdraw(tx, actor, transfer).await? {
        Ok(changed)
    } else {
        withdrawal::request(tx, actor, transfer, now).await
    }
}

pub(in crate::fleet::agent_threads) async fn retained_sources(
    ctx: &Context<'_>,
    destination: DeviceId,
    principal: Uuid,
) -> Result<Vec<Value>> {
    retirement::pending(pool(ctx)?, destination, principal)
        .await
        .map_err(failed)
}

pub(in crate::fleet::agent_threads) async fn status(
    ctx: &Context<'_>,
    transfer: Uuid,
    principal: Uuid,
) -> Result<Option<Value>> {
    placement::status(
        pool(ctx)?,
        transfer,
        principal,
        Trail::from_context(ctx)?.now,
    )
    .await
    .map_err(failed)
}

fn failed(error: Error) -> async_graphql::Error {
    match error {
        Error::Refused(message) => coded(message, ErrorCode::Conflict),
        error @ Error::Database(_) => internal(error),
    }
}
