//! Transcript uploads use the daemon credential and its turn without marking metrics fresh.

use std::net::IpAddr;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use chrono::{DateTime, Utc};
use daemon_core::wire::{MAX_THREAD_UPLOAD, RefusalCode, ThreadUpload, ThreadUploadReceipt};
use uuid::Uuid;

use super::super::{
    Error,
    admission::{Admitted, Stop, admit},
    credential,
    refusals::{self, Route},
    routes::{DaemonState, busy, client_of, read_body, unmounted, within},
    store,
    turns::Turn,
};
use crate::fleet::{
    agent_threads,
    devices::{self, MetricsSource},
};

pub(in crate::fleet::metrics::ingest) async fn serve(
    State(state): State<DaemonState>,
    request: Request,
) -> Response {
    within(state.deadlines.answer, receive(state, request)).await
}

async fn receive(state: DaemonState, request: Request) -> Response {
    let Some(client) = client_of(&request) else {
        return unmounted();
    };
    let now = state.clock.now();
    let Some(address) = state.turns.address(client.0) else {
        return busy();
    };
    let (parts, body) = request.into_parts();
    match upload(&state, (&parts.headers, body), address, now).await {
        Ok(receipt) => refusals::json(StatusCode::OK, &receipt),
        Err(stop) => {
            state
                .stopped(stop, (Route::ThreadUpload, client), now)
                .await
        }
    }
}

async fn upload(
    state: &DaemonState,
    (headers, body): (&HeaderMap, Body),
    address: Turn<IpAddr>,
    now: DateTime<Utc>,
) -> Result<ThreadUploadReceipt, Stop> {
    let token =
        credential::bearer(headers).ok_or(Stop::Refused(RefusalCode::TokenRefused, None))?;
    let Admitted {
        device,
        turn: _turn,
        ..
    } = admit(state, &token, address, now).await?;
    let refused = |code| Stop::Refused(code, Some(device));
    let body = read_body(state, headers, body, MAX_THREAD_UPLOAD)
        .await
        .map_err(refused)?;
    let request: ThreadUpload =
        serde_json::from_slice(&body).map_err(|_| refused(RefusalCode::Malformed))?;
    if request.device != Uuid::from(device) {
        return Err(refused(RefusalCode::WrongDevice));
    }
    if !request.fits() {
        return Err(refused(RefusalCode::Malformed));
    }
    let _request = state.turns.request().await.ok_or(Stop::Busy)?;
    let mut tx = state.db.begin().await.map_err(failed)?;
    match devices::lock_metrics_source(&mut tx, device).await {
        Ok(MetricsSource::Daemon) => (),
        Ok(_) | Err(devices::Error::NotFound(_)) => return Err(refused(RefusalCode::TokenRefused)),
        Err(error) => return Err(Stop::Failed(Error::Devices(error))),
    }
    if !store::lock_token(&mut tx, device, token.hash(), now)
        .await
        .map_err(failed)?
    {
        return Err(refused(RefusalCode::TokenRefused));
    }
    let receipt = agent_threads::append(&mut tx, device, &request)
        .await
        .map_err(failed)?
        .ok_or_else(|| refused(RefusalCode::Malformed))?;
    tx.commit().await.map_err(failed)?;
    Ok(receipt)
}

fn failed(error: sqlx::Error) -> Stop {
    Stop::Failed(Error::Database(error))
}
