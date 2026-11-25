//! Authenticated K10 communication with the holder of a remotely started thread.

use std::time::Duration;

use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
};
use chrono::{DateTime, Utc};
use daemon_core::wire::{DaemonToken, RefusalCode};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::json;
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

use super::{
    credential, refusals,
    routes::{DaemonState, busy, client_of, read_body, unmounted, within},
    store,
};
use crate::{
    fleet::{
        agent_threads::remote::{self, DispatchDecision, Error, Owner},
        devices::{self, MetricsSource},
        events::{ClientAddr, Operator},
    },
    ids::DeviceId,
};

pub(super) fn routes() -> Router<DaemonState> {
    Router::new()
        .route("/work/v1/thread-dispatch", post(dispatch))
        .route("/work/v1/thread-launch-evidence", post(evidence))
        .route("/work/v1/thread-control", post(control))
        .route("/work/v1/commands", post(commands))
        .route("/work/v1/command-result", post(command_result))
}

pub(super) struct Holder {
    pub device: DeviceId,
    pub token: DaemonToken,
}

impl Holder {
    pub(super) async fn locked<'a>(
        &self,
        state: &'a DaemonState,
    ) -> Result<(Transaction<'a, Postgres>, DateTime<Utc>), Error> {
        let mut tx = state.db.begin().await?;
        match devices::lock_metrics_source(&mut tx, self.device).await {
            Ok(MetricsSource::Daemon) => {}
            Ok(_) | Err(devices::Error::NotFound(_)) => {
                return Err(Error::Refused("TOKEN_REFUSED"));
            }
            Err(devices::Error::Database(error)) => return Err(error.into()),
            Err(_) => return Err(Error::Refused("UNAVAILABLE")),
        }
        if !store::lock_token(&mut tx, self.device, self.token.hash(), state.clock.now()).await? {
            return Err(Error::Refused("TOKEN_REFUSED"));
        }
        let at = sqlx::query_scalar("SELECT clock_timestamp()")
            .fetch_one(&mut *tx)
            .await?;
        Ok((tx, at))
    }

    pub(super) async fn owner(&self, conn: &mut PgConnection) -> Result<Owner, Error> {
        Ok(Owner {
            account: remote::operator(conn).await?,
            device: self.device.into(),
        })
    }
}

enum EdgeError {
    Unmounted,
    Busy,
    Refused(RefusalCode),
}

impl IntoResponse for EdgeError {
    fn into_response(self) -> Response {
        match self {
            Self::Unmounted => unmounted(),
            Self::Busy => busy(),
            Self::Refused(code) => refusals::answer(code),
        }
    }
}

async fn holder(state: &DaemonState, headers: &HeaderMap) -> Result<Holder, EdgeError> {
    let token = credential::bearer(headers).ok_or(EdgeError::Refused(RefusalCode::TokenRefused))?;
    let device = credential::authenticate(&state.db, &token, state.clock.now())
        .await
        .map_err(|_| EdgeError::Busy)?
        .ok_or(EdgeError::Refused(RefusalCode::TokenRefused))?;
    Ok(Holder { device, token })
}

async fn request<T: DeserializeOwned>(
    state: &DaemonState,
    request: Request,
    limit: usize,
) -> Result<(Holder, T, ClientAddr), EdgeError> {
    let client = client_of(&request).ok_or(EdgeError::Unmounted)?;
    let address = state.turns.address(client.0).ok_or(EdgeError::Busy)?;
    let (parts, body) = request.into_parts();
    let holder = holder(state, &parts.headers).await?;
    drop(address);
    let bytes = read_body(state, &parts.headers, body, limit)
        .await
        .map_err(EdgeError::Refused)?;
    let body =
        serde_json::from_slice(&bytes).map_err(|_| EdgeError::Refused(RefusalCode::Malformed))?;
    Ok((holder, body, client))
}

pub(super) fn response(error: &Error) -> Response {
    let Error::Refused(code) = error else {
        return busy();
    };
    let status = match *code {
        "TOKEN_REFUSED" => StatusCode::UNAUTHORIZED,
        "WORK_OFF" | "WRONG_DEVICE" => StatusCode::FORBIDDEN,
        "NOT_HOLDER" | "LAUNCH_CONFLICT" | "CONTROL_REVISION_CONFLICT" | "target_unavailable" => {
            StatusCode::CONFLICT
        }
        "LAUNCH_UNKNOWN" | "COMMAND_UNKNOWN" | "THREAD_NOT_FOUND" => StatusCode::NOT_FOUND,
        "MALFORMED" => StatusCode::BAD_REQUEST,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };
    refusals::json(status, &json!({"error": code}))
}

macro_rules! endpoint {
    ($name:ident, $limit:expr, |$conn:ident, $owner:ident, $body:ident, $at:ident, $client:ident| $run:block) => {
        async fn $name(State(state): State<DaemonState>, inbound: Request) -> Response {
            within(state.deadlines.answer, async {
                let (holder, $body, $client) = match request(&state, inbound, $limit).await {
                    Ok(request) => request,
                    Err(error) => return error.into_response(),
                };
                let Some(_turn) = state.turns.device(holder.device) else {
                    return busy();
                };
                let Some(_request) = state.turns.request().await else {
                    return busy();
                };
                let outcome: Result<Response, Error> = async {
                    let (mut tx, $at) = holder.locked(&state).await?;
                    let $owner = holder.owner(&mut tx).await?;
                    let $conn = &mut *tx;
                    let answer = $run;
                    tx.commit().await?;
                    Ok(answer)
                }
                .await;
                outcome.unwrap_or_else(|error| response(&error))
            })
            .await
        }
    };
}

endpoint!(dispatch, 4096, |conn, owner, body, at, _client| {
    match remote::dispatch(conn, owner, body, at).await? {
        DispatchDecision::Granted(grant) => refusals::json(StatusCode::OK, &grant),
        DispatchDecision::Reconcile(authorization) => {
            let mut body = json!({"error": "NOT_HOLDER"});
            if let Some(authorization) = authorization {
                body["authorization"] = json!(authorization);
            }
            refusals::json(StatusCode::CONFLICT, &body)
        }
        DispatchDecision::Denied(cause) => response(&Error::Refused(cause)),
    }
});

endpoint!(evidence, 4096, |conn, owner, body, at, client| {
    let receipt = remote::evidence(conn, (owner, at), body, (Operator::hub(), client)).await?;
    refusals::json(StatusCode::OK, &receipt)
});

endpoint!(control, 65536, |conn, owner, body, at, _client| {
    remote::upload(conn, owner, body, at).await?;
    StatusCode::NO_CONTENT.into_response()
});

endpoint!(command_result, 4096, |conn, owner, body, at, client| {
    remote::result(conn, (owner, at), body, (Operator::hub(), client)).await?;
    StatusCode::OK.into_response()
});

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Poll {
    device: Uuid,
}

async fn commands(State(state): State<DaemonState>, inbound: Request) -> Response {
    let answer = async {
        let (holder, body, _client) = request::<Poll>(&state, inbound, 8192)
            .await
            .map_err(IntoResponse::into_response)?;
        if body.device != Uuid::from(holder.device) {
            return Err(refusals::answer(RefusalCode::WrongDevice));
        }
        let Some(_poll) = state.turns.command_poll(holder.device) else {
            return Err(busy());
        };
        let delivered =
            tokio::time::timeout(Duration::from_secs(25), poll_until(&state, &holder)).await;
        let Ok(delivered) = delivered else {
            return Ok(StatusCode::NO_CONTENT.into_response());
        };
        let delivered = delivered.map_err(|error| response(&error))?;
        Ok(refusals::json(
            StatusCode::OK,
            &json!({"commands": delivered}),
        ))
    };
    answer.await.unwrap_or_else(|refused| refused)
}

async fn poll_until(
    state: &DaemonState,
    holder: &Holder,
) -> Result<impl serde::Serialize + use<>, Error> {
    loop {
        let (mut tx, at) = holder.locked(state).await?;
        let owner = holder.owner(&mut tx).await?;
        let delivered = remote::poll(&mut tx, owner, at).await?;
        tx.commit().await?;
        if !delivered.is_empty() {
            return Ok(delivered);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}
