//! The device turn covers reservation and release, leaving GitHub I/O outside both.
use std::net::IpAddr;

use agent_threads::managed::workspace::CheckoutRequest;
use axum::{body::Body, http::HeaderMap, response::Response};
use chrono::{DateTime, Utc};
use daemon_core::wire::DaemonToken;
use sqlx::PgConnection;

use super::super::{Error, admission::Stop, credential, routes::DaemonState, store, turns::Turn};
use super::{
    Asked, SECRET_LIMIT, asked, failed, secret_response,
    secret_route::{readmit, refused},
};
use crate::{
    fleet::{
        devices::{self, MetricsSource},
        events::ClientAddr,
    },
    ids::DeviceId,
    work::checkout::{
        Issuer, RepositoryToken,
        workspace::{self, Ticket},
    },
};

pub(super) async fn serve(
    state: &DaemonState,
    (headers, body): (&HeaderMap, Body),
    (address, client, issuer): (Turn<IpAddr>, ClientAddr, Option<Issuer>),
    now: DateTime<Utc>,
) -> Result<Response, Stop> {
    let asked: Asked<CheckoutRequest> =
        asked(state, (headers, body), address, (SECRET_LIMIT, now)).await?;
    let issuer = issuer.ok_or_else(|| refused(asked.device))?;
    let credential = credential::bearer(headers).ok_or_else(|| refused(asked.device))?;
    let ticket = {
        let _turn = state.turns.request().await.ok_or(Stop::Busy)?;
        let mut tx = state.db.begin().await.map_err(database)?;
        let now = state.clock.now();
        if enrolled_until(&mut tx, asked.device, &credential, now)
            .await?
            .is_none()
        {
            return Err(refused(asked.device));
        }
        let ticket = workspace::reserve(&mut tx, asked.device, &asked.body, now)
            .await
            .map_err(failed)?
            .ok_or_else(|| refused(asked.device))?;
        tx.commit().await.map_err(database)?;
        ticket
    };
    let device = asked.device;
    drop(asked);
    tokio::spawn(issue(
        state.clone(),
        issuer,
        (device, client, credential),
        ticket,
    ))
    .await
    .map_err(|_| Stop::Busy)?
}

async fn issue(
    state: DaemonState,
    issuer: Issuer,
    (device, client, credential): (DeviceId, ClientAddr, DaemonToken),
    ticket: Ticket,
) -> Result<Response, Stop> {
    let token = issuer
        .mint_workspace(&ticket)
        .await
        .map_err(|_| refused(device))?;
    let retained = tokio::time::timeout(
        state.deadlines.request,
        issuer.retain_workspace_token((&ticket, &token)),
    )
    .await;
    match retained {
        Ok(Ok(())) => (),
        failure => {
            issuer.revoke_unrecorded(&token).await;
            return Err(failure
                .ok()
                .and_then(Result::err)
                .map_or(Stop::Busy, failed));
        }
    }
    let admitted = readmit(&state, &credential, (device, client)).await;
    let turn = if admitted.is_ok() {
        state.turns.request().await
    } else {
        None
    };
    let auth = turn.as_ref().map(|_| (device, client, &credential));
    let limit = state.deadlines.answer + state.deadlines.request;
    let outcome = tokio::time::timeout(limit, settle(&state, &issuer, (&ticket, &token), auth))
        .await
        .unwrap_or(Err(Stop::Busy));
    match outcome {
        Ok(Some(expiry)) => Ok(secret_response::answer(&token.value, expiry)),
        Ok(None) => Err(admitted.err().unwrap_or_else(|| refused(device))),
        Err(stop) => Err(stop),
    }
}

async fn settle(
    state: &DaemonState,
    issuer: &Issuer,
    (ticket, token): (&Ticket, &RepositoryToken),
    auth: Option<(DeviceId, ClientAddr, &DaemonToken)>,
) -> Result<Option<DateTime<Utc>>, Stop> {
    let mut tx = state.db.begin().await.map_err(database)?;
    let allowed = if let Some((device, client, credential)) = auth {
        enrolled_until(&mut tx, device, credential, state.clock.now())
            .await?
            .map(|ends| (client, ends))
    } else {
        None
    };
    let expires = issuer
        .settle_workspace(&mut tx, (ticket, token), allowed)
        .await
        .map_err(failed)?;
    let now = state.clock.now();
    if expires.is_some_and(|end| end <= now || !ticket.live(now)) {
        tx.rollback().await.map_err(database)?;
        let mut denied = state.db.begin().await.map_err(database)?;
        issuer
            .settle_workspace(&mut denied, (ticket, token), None)
            .await
            .map_err(failed)?;
        denied.commit().await.map_err(database)?;
        return Ok(None);
    }
    tx.commit().await.map_err(database)?;
    Ok(expires)
}

async fn enrolled_until(
    tx: &mut PgConnection,
    device: DeviceId,
    token: &DaemonToken,
    now: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>, Stop> {
    match devices::lock_metrics_source(tx, device).await {
        Ok(MetricsSource::Daemon) => store::lock_token_until(tx, device, token.hash(), now)
            .await
            .map_err(database),
        Ok(_) | Err(devices::Error::NotFound(_)) => Ok(None),
        Err(error) => Err(Stop::Failed(Error::Devices(error))),
    }
}

fn database(error: sqlx::Error) -> Stop {
    Stop::Failed(Error::Database(error))
}
