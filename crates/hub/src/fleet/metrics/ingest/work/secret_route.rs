//! Keep credential minting outside the device and database turns.
use std::{net::IpAddr, time::Duration};

use axum::{body::Body, http::HeaderMap, response::Response};
use chrono::{DateTime, Utc};
use daemon_core::wire::{DaemonToken, REPOSITORY_TOKEN, RefusalCode, SecretRequest};

use super::super::{
    admission::{Admitted, Stop, admit},
    credential,
    routes::DaemonState,
    turns::Turn,
};
use super::{Asked, SECRET_LIMIT, asked, failed, secret_response};
use crate::{
    fleet::events::ClientAddr,
    ids::DeviceId,
    work::{
        checkout,
        jobs::{self, Queue},
    },
};

pub(super) async fn serve(
    state: &DaemonState,
    (headers, body): (&HeaderMap, Body),
    (address, client, issuer): (Turn<IpAddr>, ClientAddr, Option<checkout::Issuer>),
    now: DateTime<Utc>,
) -> Result<Response, Stop> {
    let asked: Asked<SecretRequest> =
        asked(state, (headers, body), address, (SECRET_LIMIT, now)).await?;
    if asked.body.purpose != REPOSITORY_TOKEN {
        return application_file(state, client, &asked).await;
    }
    let issuer = issuer.ok_or_else(|| refused(asked.device))?;
    let credential = credential::bearer(headers).ok_or_else(|| refused(asked.device))?;
    let ticket = {
        let _request = state.turns.request().await.ok_or(Stop::Busy)?;
        issuer
            .start(asked.device, &asked.body)
            .await
            .map_err(failed)?
            .ok_or_else(|| refused(asked.device))?
    };
    let device = asked.device;
    drop(asked);
    let state = state.clone();
    // A lost HTTP reply still consumes the attempt's single credential release.
    tokio::spawn(issue(state, issuer, (credential, device, client), ticket))
        .await
        .map_err(|_| Stop::Busy)?
}

async fn application_file(
    state: &DaemonState,
    client: ClientAddr,
    asked: &Asked<SecretRequest>,
) -> Result<Response, Stop> {
    let _request = state.turns.request().await.ok_or(Stop::Busy)?;
    let queue = Queue {
        db: &state.db,
        bus: &state.bus,
    };
    let file = jobs::take_application_file(
        (queue, &state.vault, state.clock.as_ref()),
        (asked.device, client),
        &asked.body,
    )
    .await
    .map_err(failed)?
    .ok_or_else(|| refused(asked.device))?;
    Ok(secret_response::answer(&file.value, file.expires_at))
}

async fn issue(
    state: DaemonState,
    issuer: checkout::Issuer,
    (credential, device, client): (DaemonToken, DeviceId, ClientAddr),
    ticket: checkout::Ticket,
) -> Result<Response, Stop> {
    let token = issuer.mint(&ticket).await.map_err(|_| refused(device))?;
    let authorization = readmit(&state, &credential, (device, client)).await;
    let database_turn = state.turns.request().await;
    let can_release = authorization.is_ok() && database_turn.is_some();
    let result = issuer
        .finish(ticket, &token, can_release.then_some(client))
        .await;
    drop(database_turn);
    let auth_refusal = authorization.err();
    // A failed commit may be ambiguous. Try revocation while returning only the refusal.
    if result.is_err() {
        issuer.revoke_unrecorded(&token).await;
    }
    let expiry = result.map_err(failed)?;
    if let Some(stop) = auth_refusal {
        return Err(stop);
    }
    expiry
        .map(|time| secret_response::answer(&token.value, time))
        .ok_or_else(|| refused(device))
}

pub(super) async fn readmit(
    state: &DaemonState,
    credential: &DaemonToken,
    (device, client): (DeviceId, ClientAddr),
) -> Result<Admitted, Stop> {
    let admitted = tokio::time::timeout(state.deadlines.request, async {
        loop {
            if let Some(address) = state.turns.address(client.0) {
                match admit(state, credential, address, state.clock.now()).await {
                    Err(Stop::Busy) => {}
                    result => return result,
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| Stop::Busy)??;
    if admitted.device != device {
        return Err(Stop::Refused(RefusalCode::TokenRefused, None));
    }
    Ok(admitted)
}

pub(super) fn refused(device: DeviceId) -> Stop {
    Stop::Refused(RefusalCode::SecretRefused, Some(device))
}
