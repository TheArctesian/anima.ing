use super::{Error, MoveRequest, Start, placement};
use crate::{
    clock::Clock,
    fleet::{
        access::{self, TokenKind},
        events::{self, ActorKind, AuditAction, AuditTarget, NewAudit, Operator},
    },
    ids::DeviceId,
};
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::json;
use sqlx::PgPool;
use std::{sync::Arc, time::Duration};
use uuid::Uuid;

#[derive(Clone)]
struct Workspaces {
    db: PgPool,
    clock: Arc<dyn Clock>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct NamedMove {
    transfer: Uuid,
    workspace: Uuid,
    source_device: DeviceId,
    destination_device: DeviceId,
    generation: u64,
    storage_bytes: u64,
    #[serde(default)]
    continue_session: bool,
}

impl NamedMove {
    fn start(&self) -> Start {
        Start {
            request: MoveRequest {
                transfer: self.transfer,
                workspace: self.workspace,
                source_device: self.source_device,
                generation: self.generation,
                storage_bytes: self.storage_bytes,
                continue_session: self.continue_session,
            },
            destination_device: self.destination_device,
            headroom: None,
        }
    }
}

pub(crate) fn router(db: PgPool, clock: Arc<dyn Clock>) -> Router {
    Router::new()
        .route("/workspaces/v1/handoffs", post(transfer))
        .route("/workspaces/v1/handoffs/{transfer}", get(status))
        .with_state(Workspaces { db, clock })
}

async fn transfer(
    State(state): State<Workspaces>,
    request: Request,
) -> Result<Response, StatusCode> {
    let (parts, body) = request.into_parts();
    let operator = signed_in(&state, &parts.headers).await?;
    let client = access::client_addr(&parts.extensions).ok_or(StatusCode::FORBIDDEN)?;
    let request = tokio::time::timeout(Duration::from_secs(5), to_bytes(body, 4096))
        .await
        .map_err(|_| StatusCode::REQUEST_TIMEOUT)?
        .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)?;
    let start: NamedMove = serde_json::from_slice(&request).map_err(|_| StatusCode::BAD_REQUEST)?;
    let current = signed_in(&state, &parts.headers).await?;
    if operator.id != current.id {
        return Err(StatusCode::FORBIDDEN);
    }
    let now = state.clock.now();
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    match placement::request_named(&mut tx, operator.id.into(), start.start(), now).await {
        Ok(true) => events::audit(&mut tx, now, NewAudit {
            operator, action: AuditAction::Update,
            target: AuditTarget::Device(start.source_device), client,
            change: json!({"workspaceHandoff":start.transfer,"workspace":start.workspace,"destinationDevice":start.destination_device}),
        }).await.map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
        Ok(false) => (),
        Err(Error::Refused(reason)) => return Ok((StatusCode::CONFLICT, Json(json!({"error":reason}))).into_response()),
        Err(_) => return Err(StatusCode::SERVICE_UNAVAILABLE),
    }
    tx.commit()
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok((
        StatusCode::ACCEPTED,
        [(header::CACHE_CONTROL, "no-store")],
        Json(json!({"transfer":start.transfer})),
    )
        .into_response())
}

async fn status(
    State(state): State<Workspaces>,
    Path(transfer): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    let operator = signed_in(&state, &headers).await?;
    let status = placement::status(&state.db, transfer, operator.id.into(), state.clock.now())
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(status)).into_response())
}

async fn signed_in(state: &Workspaces, headers: &HeaderMap) -> Result<Operator, StatusCode> {
    let identity = access::identify(&state.db, headers, None, state.clock.now())
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    identity
        .operator()
        .filter(|operator| {
            identity.kind() == TokenKind::Login && operator.actor.kind() == ActorKind::User
        })
        .cloned()
        .ok_or(StatusCode::FORBIDDEN)
}
