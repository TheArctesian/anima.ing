//! `POST /work/v1/claim`, `POST /work/v1/report` and `POST /work/v1/secret`: the work route
//! (K10, `contracts/work-route.md`, accepted by R-209). The transport, the credential, the
//! limits and the refusals are those of the daemon routes of K8: the daemon token is the
//! credential, and the checks run in the order of a renewal. The rules of a job are in
//! `work::jobs`; this file only reads a request and writes an answer.
//!
//! `/work/v1/secret` answers `SECRET_REFUSED` to each request that passes the credential and
//! the form: no purpose of K10 Section 8 has a source in the hub yet (tasks B2 and A2).

mod threads;

use std::net::IpAddr;

use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::post,
};
use chrono::{DateTime, Utc};
use daemon_core::wire::{DaemonToken, RefusalCode, SecretRequest, WorkReport};
use serde::de::DeserializeOwned;

use super::{
    Error,
    admission::{Admitted, Stop, admit},
    credential, refusals,
    refusals::Route,
    remote_threads,
    routes::{DaemonState, busy, client_of, read_body, unmounted, within},
    turns::Turn,
};
use crate::{
    fleet::events::ClientAddr,
    ids::DeviceId,
    work::jobs::{self, Queue, Reporting},
};

/// The body limits of K10 Section 2.
const CLAIM_LIMIT: usize = 8 * 1024;
const REPORT_LIMIT: usize = 96 * 1024;
const SECRET_LIMIT: usize = 4 * 1024;

/// A request of a daemon whose token is usable, with its body read by the form `T`.
struct Asked<T> {
    device: DeviceId,
    name: String,
    body: T,
    token: DaemonToken,
    /// The turn of the device, held until the answer is made (K8 Section 6).
    _turn: Turn<DeviceId>,
}

/// The bearer before the body, the size, then the JSON (as a renewal).
async fn asked<T: DeserializeOwned>(
    state: &DaemonState,
    (headers, body): (&HeaderMap, Body),
    address: Turn<IpAddr>,
    (limit, now): (usize, DateTime<Utc>),
) -> Result<Asked<T>, Stop> {
    let token =
        credential::bearer(headers).ok_or(Stop::Refused(RefusalCode::TokenRefused, None))?;
    let Admitted { device, name, turn } = admit(state, &token, address, now).await?;
    let refused = |code| Stop::Refused(code, Some(device));
    let bytes = read_body(state, headers, body, limit)
        .await
        .map_err(refused)?;
    let body = serde_json::from_slice::<T>(&bytes).map_err(|_| refused(RefusalCode::Malformed))?;
    Ok(Asked {
        device,
        name,
        body,
        token,
        _turn: turn,
    })
}

/// What each of the three routes does before its own work, and how it answers a stop.
macro_rules! served {
    ($state:ident, $request:ident, $route:expr, $answer:ident) => {{
        let Some(client) = client_of(&$request) else {
            return unmounted();
        };
        let now = $state.clock.now();
        let Some(address) = $state.turns.address(client.0) else {
            return busy();
        };
        let (parts, body) = $request.into_parts();
        match $answer(&$state, (&parts.headers, body), (address, client), now).await {
            Ok(response) => response,
            Err(stop) => $state.stopped(stop, ($route, client), now).await,
        }
    }};
}

/// The three routes of K10, each answering within the deadline of a daemon route.
pub(super) fn routes() -> Router<DaemonState> {
    Router::new()
        .route("/work/v1/claim", post(claim))
        .route("/work/v1/report", post(report))
        .route("/work/v1/secret", post(secret_of_job))
}

/// Answers a claim: `200` with a job, `204` with none, or the first refusal.
async fn claim(State(state): State<DaemonState>, request: Request) -> Response {
    let limit = state.deadlines.answer;
    within(limit, async move {
        served!(state, request, Route::WorkClaim, claimed)
    })
    .await
}

/// Answers a report: `200` with the new lease, or the first refusal.
async fn report(State(state): State<DaemonState>, request: Request) -> Response {
    let limit = state.deadlines.answer;
    within(limit, async move {
        served!(state, request, Route::WorkReport, reported)
    })
    .await
}

/// Answers a request for a secret of a job: a refusal (see the head of this file).
async fn secret_of_job(State(state): State<DaemonState>, request: Request) -> Response {
    let limit = state.deadlines.answer;
    within(limit, async move {
        served!(state, request, Route::WorkSecret, secret)
    })
    .await
}

fn failed(error: jobs::Error) -> Stop {
    Stop::Failed(Error::Jobs(error))
}

async fn claimed(
    state: &DaemonState,
    request: (&HeaderMap, Body),
    (address, client): (Turn<IpAddr>, ClientAddr),
    now: DateTime<Utc>,
) -> Result<Response, Stop> {
    let asked: Asked<serde_json::Value> =
        asked(state, request, address, (CLAIM_LIMIT, now)).await?;
    let _request = state.turns.request().await.ok_or(Stop::Busy)?;
    let holder = remote_threads::Holder {
        device: asked.device,
        token: asked.token,
    };
    Ok(threads::claim(state, holder, (asked.body, &asked.name), client).await)
}

async fn reported(
    state: &DaemonState,
    request: (&HeaderMap, Body),
    (address, client): (Turn<IpAddr>, ClientAddr),
    now: DateTime<Utc>,
) -> Result<Response, Stop> {
    let asked: Asked<serde_json::Value> =
        asked(state, request, address, (REPORT_LIMIT, now)).await?;
    let device = asked.device;
    let refused = |code| Stop::Refused(code, Some(device));
    let _request = state.turns.request().await.ok_or(Stop::Busy)?;
    let holder = remote_threads::Holder {
        device,
        token: asked.token,
    };
    match threads::report(state, holder, &asked.body).await {
        Ok(Some(response)) => return Ok(response),
        Ok(None) => {}
        Err(error) => return Ok(remote_threads::response(&error)),
    }
    let report: WorkReport =
        serde_json::from_value(asked.body).map_err(|_| refused(RefusalCode::Malformed))?;
    let queue = Queue {
        db: &state.db,
        bus: &state.bus,
    };
    match jobs::report(queue, device, &report, (client, now))
        .await
        .map_err(failed)?
    {
        Reporting::Reported(answer) => Ok(refusals::json(StatusCode::OK, &answer)),
        Reporting::NotHolder => Err(refused(RefusalCode::NotHolder)),
        Reporting::Malformed => Err(refused(RefusalCode::Malformed)),
        Reporting::Off => Err(refused(RefusalCode::WorkOff)),
    }
}

async fn secret(
    state: &DaemonState,
    request: (&HeaderMap, Body),
    (address, _client): (Turn<IpAddr>, ClientAddr),
    now: DateTime<Utc>,
) -> Result<Response, Stop> {
    let asked: Asked<SecretRequest> = asked(state, request, address, (SECRET_LIMIT, now)).await?;
    Err(Stop::Refused(
        RefusalCode::SecretRefused,
        Some(asked.device),
    ))
}
