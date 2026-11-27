//! Enrolled-device workspace transfer. The body never holds a device request turn.
mod access;
mod download;
mod restore;

use super::{
    admission::Stop,
    routes::{DaemonState, busy, read_body, within},
};
use crate::{
    fleet::agent_threads::handoffs as custody,
    work::artifacts::{Error, Store, checkpoints::Pending},
};
use access::{Access, custody_error};
use agent_threads::managed::workspace::handoff::Receipt;
use axum::{
    Extension, Router,
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use daemon_core::wire::RefusalCode;
use futures_util::TryStreamExt as _;
use tokio_util::io::StreamReader;
use uuid::Uuid;

const RECEIPT_HEADER: &str = "anima-workspace-checkpoint";

pub(super) fn routes() -> Router<DaemonState> {
    Router::new()
        .route("/daemon/v1/workspace-handoffs/poll", post(poll))
        .route(
            "/daemon/v1/workspace-handoffs/{transfer}/checkpoint",
            post(upload).merge(get(download::serve)),
        )
        .route(
            "/daemon/v1/workspace-handoffs/{transfer}/confirm",
            post(confirm),
        )
        .route(
            "/daemon/v1/workspace-handoffs/{transfer}/restored",
            post(restore::restored),
        )
        .route(
            "/daemon/v1/workspace-handoffs/{transfer}/activated",
            post(restore::activated),
        )
        .route(
            "/daemon/v1/workspace-handoffs/{transfer}/withdrawal",
            post(restore::withdrawal),
        )
        .route(
            "/daemon/v1/workspace-handoffs/{transfer}/retired",
            post(restore::retired),
        )
}

async fn poll(
    State(state): State<DaemonState>,
    store: Option<Extension<Store>>,
    request: Request,
) -> Response {
    let Some(Extension(_)) = store else {
        return busy();
    };
    within(state.deadlines.answer, serve_poll(state, request)).await
}

async fn serve_poll(state: DaemonState, request: Request) -> Response {
    let (access, _, _) = match Access::open(state, request).await {
        Ok(parts) => parts,
        Err(response) => return *response,
    };
    let outcome = async {
        let mut guard = access.lock().await?;
        let offer = custody::issued(&mut guard.tx, guard.device, access.now())
            .await
            .map_err(|error| custody_error(guard.device, error))?;
        guard.commit().await?;
        Ok(offer.map_or_else(
            || StatusCode::NO_CONTENT.into_response(),
            |offer| super::refusals::json(StatusCode::OK, &offer),
        ))
    }
    .await;
    access.answer(outcome).await
}

async fn upload(
    State(state): State<DaemonState>,
    Path(id): Path<Uuid>,
    store: Option<Extension<Store>>,
    request: Request,
) -> Response {
    let Some(Extension(store)) = store else {
        return busy();
    };
    let (access, parts, body) = match Access::open(state, request).await {
        Ok(parts) => parts,
        Err(response) => return *response,
    };
    let outcome = async {
        let mut guard = access.lock().await?;
        let device = guard.device;
        let receipt = header_receipt(&parts.headers, id)
            .ok_or(Stop::Refused(RefusalCode::Malformed, Some(device)))?;
        let row = custody::prepare(&mut guard.tx, device, &receipt, access.now())
            .await
            .map_err(|error| custody_error(device, error))?;
        let import = row
            .import(&receipt)
            .ok_or(Stop::Refused(RefusalCode::NotHolder, Some(device)))?;
        guard.commit().await?;
        let input = StreamReader::new(body.into_data_stream().map_err(std::io::Error::other));
        let pending = store
            .receive_checkpoint(import, (access.now(), row.expires_at), input)
            .await
            .map_err(|error| storage_error(device, &error))?;
        let receipt = tokio::spawn(stage(access.clone(), receipt, device, pending))
            .await
            .map_err(|_| Stop::Busy)??;
        Ok(super::refusals::json(StatusCode::ACCEPTED, &receipt))
    }
    .await;
    access.answer(outcome).await
}

async fn stage(
    access: Access,
    receipt: Receipt,
    device: crate::ids::DeviceId,
    pending: Pending,
) -> Result<Receipt, Stop> {
    let mut guard = access.lock().await?;
    if guard.device != device {
        return Err(Stop::Refused(RefusalCode::TokenRefused, None));
    }
    let row = custody::prepare(&mut guard.tx, device, &receipt, access.now())
        .await
        .map_err(|error| custody_error(device, error))?;
    if !pending
        .retain(access.clock(), guard.expires.min(row.expires_at))
        .await
        .map_err(|error| storage_error(device, &error))?
    {
        return Err(Stop::Refused(RefusalCode::NotHolder, Some(device)));
    }
    custody::staged(&mut guard.tx, &receipt)
        .await
        .map_err(|error| custody_error(device, error))?;
    guard.commit().await?;
    Ok(receipt)
}

async fn confirm(
    State(state): State<DaemonState>,
    Path(id): Path<Uuid>,
    store: Option<Extension<Store>>,
    request: Request,
) -> Response {
    let Some(Extension(store)) = store else {
        return busy();
    };
    let (access, parts, body) = match Access::open(state, request).await {
        Ok(parts) => parts,
        Err(response) => return *response,
    };
    let outcome = async {
        let auth = access.lock().await?;
        let device = auth.device;
        auth.commit().await?;
        let refused = |code| Stop::Refused(code, Some(device));
        let raw = read_body(&access.state, &parts.headers, body, 4096)
            .await
            .map_err(refused)?;
        let receipt: Receipt =
            serde_json::from_slice(&raw).map_err(|_| refused(RefusalCode::Malformed))?;
        if receipt.source.binding.transfer != id {
            return Err(refused(RefusalCode::Malformed));
        }
        confirmation(&access, store, &receipt, device).await?;
        Ok(StatusCode::NO_CONTENT.into_response())
    }
    .await;
    access.answer(outcome).await
}

async fn confirmation(
    access: &Access,
    store: Store,
    receipt: &Receipt,
    device: crate::ids::DeviceId,
) -> Result<(), Stop> {
    let mut guard = access.lock().await?;
    if guard.device != device {
        return Err(Stop::Refused(RefusalCode::TokenRefused, None));
    }
    let refusal = || Stop::Refused(RefusalCode::NotHolder, Some(device));
    let row = custody::prepare(&mut guard.tx, device, receipt, access.now())
        .await
        .map_err(|error| custody_error(device, error))?;
    if !row
        .checkpoint
        .as_ref()
        .is_some_and(|held| &held.0 == receipt)
    {
        return Err(refusal());
    }
    store
        .open_checkpoint(row.import(receipt).ok_or_else(refusal)?)
        .await
        .map_err(|error| storage_error(device, &error))?;
    if access.now() >= guard.expires.min(row.expires_at) {
        return Err(refusal());
    }
    custody::confirmed(&mut guard.tx, receipt)
        .await
        .map_err(|error| custody_error(device, error))?;
    guard.commit().await
}

fn header_receipt(headers: &HeaderMap, id: Uuid) -> Option<Receipt> {
    let mut receipts = headers.get_all(RECEIPT_HEADER).iter();
    let encoded = receipts.next()?.as_bytes();
    if encoded.len() > 4096 || receipts.next().is_some() {
        return None;
    }
    let receipt: Receipt = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(encoded).ok()?).ok()?;
    let mut sizes = headers.get_all(header::CONTENT_LENGTH).iter();
    if let Some(size) = sizes.next() {
        let size: u64 = size.to_str().ok()?.parse().ok()?;
        if sizes.next().is_some() || (size != 0 && size != receipt.bytes) {
            return None;
        }
    }
    (receipt.source.binding.transfer == id).then_some(receipt)
}

fn storage_error(device: crate::ids::DeviceId, error: &Error) -> Stop {
    match error {
        Error::Conflict => Stop::Refused(RefusalCode::NotHolder, Some(device)),
        Error::Invalid => Stop::Refused(RefusalCode::Malformed, Some(device)),
        _ => Stop::Busy,
    }
}
