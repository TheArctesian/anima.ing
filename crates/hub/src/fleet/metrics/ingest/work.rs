//! `POST /work/v1/claim`, `POST /work/v1/report` and `POST /work/v1/secret`: the work route
//! (K10, `contracts/work-route.md`, accepted by R-209). The transport, the credential, the
//! limits and the refusals are those of the daemon routes of K8: the daemon token is the
//! credential, and the checks run in the order of a renewal. The rules of a job are in
//! `work::jobs`; this file only reads a request and writes an answer.
//!
//! Managed Compose files and private source checkout are the supported purposes of K10 Section 8.

#[cfg(unix)]
mod artifact_upload;
mod remote_threads;
mod secret_response;
mod secret_route;
mod workspace_secret;

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
use daemon_core::wire::{DaemonToken, RefusalCode, WorkReport};
use serde::de::DeserializeOwned;

use self::secret_route::serve as secret;
use self::workspace_secret::serve as workspace_credential;
use super::{
    Error,
    admission::{Admitted, Stop, admit},
    credential, refusals,
    refusals::Route,
    routes::{DaemonState, busy, client_of, read_body, unmounted, within},
    turns::Turn,
};
use crate::{
    fleet::events::{ClientAddr, Operator},
    ids::DeviceId,
    work::jobs::{self, Reporting},
};

#[cfg(unix)]
type ArtifactStore = crate::work::artifacts::Store;
#[cfg(not(unix))]
type ArtifactStore = ();

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
    ($state:ident, $request:ident, $route:expr, $answer:ident $(, $extra:expr)?) => {{
        let Some(client) = client_of(&$request) else {
            return unmounted();
        };
        let now = $state.clock.now();
        let Some(address) = $state.turns.address(client.0) else {
            return busy();
        };
        let (parts, body) = $request.into_parts();
        match $answer(&$state, (&parts.headers, body), (address, client $(, $extra)?), now).await {
            Ok(response) => response,
            Err(stop) => $state.stopped(stop, ($route, client), now).await,
        }
    }};
}

/// The three routes of K10, each answering within the deadline of a daemon route.
pub(super) fn routes() -> Router<DaemonState> {
    let routes = Router::new()
        .route("/work/v1/claim", post(claim))
        .route("/work/v1/report", post(report))
        .route("/work/v1/secret", post(secret_of_job))
        .merge(remote_threads::routes())
        .route("/daemon/v3/workspaces/checkout", post(secret_of_workspace));
    #[cfg(unix)]
    let routes = routes.route(daemon_core::artifact::ROUTE, post(artifact_upload::serve));
    routes
}

/// Answers a claim: `200` with a job, `204` with none, or the first refusal.
async fn claim(State(state): State<DaemonState>, request: Request) -> Response {
    #[cfg(unix)]
    let private_output = request
        .extensions()
        .get::<crate::work::artifacts::Store>()
        .is_some();
    #[cfg(not(unix))]
    let private_output = false;
    let limit = state.deadlines.answer;
    within(limit, async move {
        served!(state, request, Route::WorkClaim, claimed, private_output)
    })
    .await
}

/// Answers a report: `200` with the new lease, or the first refusal.
async fn report(State(state): State<DaemonState>, request: Request) -> Response {
    #[cfg(unix)]
    let artifacts = request.extensions().get::<ArtifactStore>().cloned();
    #[cfg(not(unix))]
    let artifacts = None;
    let limit = state.deadlines.answer;
    within(limit, async move {
        served!(state, request, Route::WorkReport, reported, artifacts)
    })
    .await
}

/// Releases a one-use checkout credential or managed application file.
async fn secret_of_job(State(state): State<DaemonState>, request: Request) -> Response {
    let sources = request
        .extensions()
        .get::<crate::work::checkout::Issuer>()
        .cloned();
    let limit = state.deadlines.answer;
    within(limit, async move {
        served!(state, request, Route::WorkSecret, secret, sources)
    })
    .await
}

async fn secret_of_workspace(State(state): State<DaemonState>, request: Request) -> Response {
    let issuer = request
        .extensions()
        .get::<crate::work::checkout::Issuer>()
        .cloned();
    let limit = state.deadlines.answer;
    within(limit, async move {
        served!(
            state,
            request,
            Route::Workspaces,
            workspace_credential,
            issuer
        )
    })
    .await
}

fn failed(error: jobs::Error) -> Stop {
    Stop::Failed(Error::Jobs(error))
}

async fn claimed(
    state: &DaemonState,
    request: (&HeaderMap, Body),
    (address, client, private_output): (Turn<IpAddr>, ClientAddr, bool),
    now: DateTime<Utc>,
) -> Result<Response, Stop> {
    let asked: Asked<serde_json::Value> =
        asked(state, request, address, (CLAIM_LIMIT, now)).await?;
    let _request = state.turns.request().await.ok_or(Stop::Busy)?;
    let holder = remote_threads::Holder {
        device: asked.device,
        token: asked.token,
    };
    Ok(threads::claim(
        state,
        holder,
        (asked.body, &asked.name),
        (client, private_output),
    )
    .await)
}

async fn reported(
    state: &DaemonState,
    request: (&HeaderMap, Body),
    (address, client, artifacts): (Turn<IpAddr>, ClientAddr, Option<ArtifactStore>),
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
    match threads::report(state, &holder, &asked.body, (Operator::hub(), client)).await {
        Ok(Some(response)) => return Ok(response),
        Ok(None) => {}
        Err(error) => return Ok(remote_threads::response(&error)),
    }
    let report: WorkReport =
        serde_json::from_value(asked.body).map_err(|_| refused(RefusalCode::Malformed))?;
    #[cfg(unix)]
    let retained = artifact_retained(artifacts.as_ref(), device, &report).await;
    #[cfg(not(unix))]
    let retained = {
        let _ = artifacts;
        false
    };
    let (mut tx, _) = match holder.locked(state).await {
        Ok(locked) => locked,
        Err(error) => return Ok(remote_threads::response(&error)),
    };
    let answer = jobs::report_in(
        &mut tx,
        &state.bus,
        (device, &report, retained),
        (client, state.clock.now()),
    )
    .await
    .map_err(failed)?;
    tx.commit().await.map_err(|error| failed(error.into()))?;
    match answer {
        Reporting::Reported(answer) => Ok(refusals::json(StatusCode::OK, &answer)),
        Reporting::NotHolder => Err(refused(RefusalCode::NotHolder)),
        Reporting::Malformed => Err(refused(RefusalCode::Malformed)),
        Reporting::Off => Err(refused(RefusalCode::WorkOff)),
    }
}

#[cfg(unix)]
async fn artifact_retained(
    store: Option<&ArtifactStore>,
    device: DeviceId,
    report: &WorkReport,
) -> bool {
    if report.state == daemon_core::wire::WorkState::Done
        && let Some(store) = store
    {
        return store
            .confirms_retention(device.into(), report.id, report.attempt)
            .await;
    }
    false
}
