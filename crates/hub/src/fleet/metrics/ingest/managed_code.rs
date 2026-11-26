//! Managed Code and workspace polling share enrolled-device admission and revocation.
use super::{
    Error,
    admission::{Admitted, Stop, admit},
    credential,
    refusals::{self, Route},
    routes::{DaemonState, busy, client_of, read_body, unmounted, within},
    store,
    turns::Turn,
};
use crate::{
    fleet::{
        agent_threads::{managed, provisioning},
        devices::{self, MetricsSource},
    },
    ids::DeviceId,
};
use agent_threads::managed::{self as code, servers::ServerPoll, workspace};
use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::post,
};
use chrono::{DateTime, Utc};
use daemon_core::wire::RefusalCode;
use std::net::IpAddr;

#[derive(Clone, Copy)]
enum Kind {
    Code,
    CodeServers,
    Workspace,
    WorkspaceCapacity { private_checkout: bool },
}

impl Kind {
    const fn settings(self) -> (Route, usize) {
        match self {
            Self::Code | Self::CodeServers => (Route::ManagedCode, code::MAX_POLL_BYTES),
            Self::Workspace | Self::WorkspaceCapacity { .. } => {
                (Route::Workspaces, workspace::MAX_POLL_BYTES)
            }
        }
    }

    fn decode(self, bytes: &[u8], device: DeviceId) -> Result<Poll, Stop> {
        let refused = |code| Stop::Refused(code, Some(device));
        let bad_json = |_| refused(RefusalCode::Malformed);
        let request = match self {
            Self::Code => Poll::Code(ServerPoll {
                poll: serde_json::from_slice(bytes).map_err(bad_json)?,
                servers: Vec::new(),
            }),
            Self::CodeServers => Poll::Code(serde_json::from_slice(bytes).map_err(bad_json)?),
            Self::Workspace => Poll::Workspace(
                serde_json::from_slice(bytes).map_err(bad_json)?,
                None,
                false,
            ),
            Self::WorkspaceCapacity { private_checkout } => {
                let value: workspace::CapacityPoll =
                    serde_json::from_slice(bytes).map_err(bad_json)?;
                if !value.capacity.fits() {
                    return Err(refused(RefusalCode::Malformed));
                }
                Poll::Workspace(
                    Box::new(workspace::Poll {
                        device: value.device,
                        ready: value.ready,
                        report: value.report,
                    }),
                    Some(value.capacity),
                    private_checkout,
                )
            }
        };
        let sender = match &request {
            Poll::Code(value) => value.poll.device,
            Poll::Workspace(value, ..) => value.device,
        };
        if sender != uuid::Uuid::from(device) {
            return Err(refused(RefusalCode::WrongDevice));
        }
        let fits = match &request {
            Poll::Code(value) => value.fits(),
            Poll::Workspace(value, ..) => value.fits(),
        };
        if !fits {
            return Err(refused(RefusalCode::Malformed));
        }
        Ok(request)
    }
}

enum Poll {
    Code(ServerPoll),
    Workspace(Box<workspace::Poll>, Option<workspace::Capacity>, bool),
}

enum Delivery {
    Code(code::Delivery),
    Workspace(workspace::Delivery),
}

pub(super) fn code_routes() -> Router<DaemonState> {
    Router::new()
        .route("/daemon/v1/code/poll", post(code))
        .route("/daemon/v2/code/poll", post(code_servers))
}

async fn code(State(state): State<DaemonState>, request: Request) -> Response {
    within(state.deadlines.answer, receive(state, request, Kind::Code)).await
}

async fn code_servers(State(state): State<DaemonState>, request: Request) -> Response {
    within(
        state.deadlines.answer,
        receive(state, request, Kind::CodeServers),
    )
    .await
}

pub(super) fn workspace_routes() -> Router<DaemonState> {
    let lifecycle = Router::new()
        .route("/daemon/v1/workspaces/poll", post(workspaces))
        .route("/daemon/v2/workspaces/poll", post(workspace_capacity))
        .route(
            "/daemon/v3/workspaces/poll",
            post(workspace_checkout_capacity),
        );
    #[cfg(unix)]
    let lifecycle = lifecycle.merge(super::handoffs::routes());
    lifecycle
}

async fn workspaces(State(state): State<DaemonState>, request: Request) -> Response {
    within(
        state.deadlines.answer,
        receive(state, request, Kind::Workspace),
    )
    .await
}

async fn workspace_capacity(State(state): State<DaemonState>, request: Request) -> Response {
    within(
        state.deadlines.answer,
        receive(
            state,
            request,
            Kind::WorkspaceCapacity {
                private_checkout: false,
            },
        ),
    )
    .await
}

async fn workspace_checkout_capacity(
    State(state): State<DaemonState>,
    request: Request,
) -> Response {
    within(
        state.deadlines.answer,
        receive(
            state,
            request,
            Kind::WorkspaceCapacity {
                private_checkout: true,
            },
        ),
    )
    .await
}

async fn receive(state: DaemonState, request: Request, kind: Kind) -> Response {
    let Some(client) = client_of(&request) else {
        return unmounted();
    };
    let now = state.clock.now();
    let Some(address) = state.turns.address(client.0) else {
        return busy();
    };
    let (parts, body) = request.into_parts();
    match poll(&state, (&parts.headers, body), address, (kind, now)).await {
        Ok(Delivery::Code(delivery)) => refusals::json(StatusCode::OK, &delivery),
        Ok(Delivery::Workspace(delivery)) => refusals::json(StatusCode::OK, &delivery),
        Err(stop) => state.stopped(stop, (kind.settings().0, client), now).await,
    }
}
async fn poll(
    state: &DaemonState,
    (headers, body): (&HeaderMap, Body),
    address: Turn<IpAddr>,
    (kind, now): (Kind, DateTime<Utc>),
) -> Result<Delivery, Stop> {
    let token =
        credential::bearer(headers).ok_or(Stop::Refused(RefusalCode::TokenRefused, None))?;
    let Admitted {
        device,
        turn: _turn,
        ..
    } = admit(state, &token, address, now).await?;
    let refused = |code| Stop::Refused(code, Some(device));
    let bytes = read_body(state, headers, body, kind.settings().1)
        .await
        .map_err(refused)?;
    let request = kind.decode(&bytes, device)?;
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
    let delivery = match request {
        Poll::Code(request) => managed::poll(&mut tx, device, request, now)
            .await
            .map(Delivery::Code)
            .map_err(|error| match error {
                managed::Error::Database(error) => failed(error),
                _ => refused(RefusalCode::Malformed),
            })?,
        Poll::Workspace(request, capacity, private_checkout) => {
            provisioning::poll(&mut tx, device, (*request, capacity, private_checkout), now)
                .await
                .map(Delivery::Workspace)
                .map_err(|error| match error {
                    provisioning::Error::Database(error) => failed(error),
                    _ => refused(RefusalCode::Malformed),
                })?
        }
    };
    tx.commit().await.map_err(failed)?;
    Ok(delivery)
}
fn failed(error: sqlx::Error) -> Stop {
    Stop::Failed(Error::Database(error))
}
