//! The push route: a device reports its own result to `/push/{token}` (CHK-003).
//! The token is the only authentication; any token that is not a live PUSH check's gets an empty 404.
//! Neither the path nor the token reaches a log or an error message (LOG-007).

pub mod own;

use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    body::to_bytes,
    extract::{Path, Query, Request, State, rejection::QueryRejection},
    http::{Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::PgPool;
use subtle::ConstantTimeEq as _;

use super::{
    Attempt, Beat, Check, Error,
    probe::{Judgement, ProbeResult, clean_message},
    scheduler::in_maintenance,
    store,
};
use crate::{
    clock::Clock,
    fleet::{
        devices,
        events::{Bus, HeartbeatSummary, Signal},
    },
};

/// A push request carries nothing the hub reads from its body.
const BODY_LIMIT: usize = 1024;
/// 32 bytes in unpadded base64url.
const TOKEN_LENGTH: usize = 43;
const PING_MAX_MS: u32 = 600_000;
/// A slower push is a stuck database; the device retries rather than wait for the edge's one-minute limit (NET-002).
const ANSWER_TIME: Duration = Duration::from_secs(10);

#[derive(Clone)]
struct PushState {
    db: PgPool,
    bus: Bus,
    clock: Arc<dyn Clock>,
    answer_time: Duration,
}

pub use own::{OwnCheck, OwnReport};

/// The caller mounts it behind the permitted-networks layer (NET-002) and inside `crate::logged`, which redacts the path (LOG-007).
pub fn push_router(db: PgPool, bus: Bus, clock: Arc<dyn Clock>) -> Router {
    push_router_within(db, bus, clock, ANSWER_TIME)
}

pub(in crate::fleet::checks) fn push_router_within(
    db: PgPool,
    bus: Bus,
    clock: Arc<dyn Clock>,
    answer_time: Duration,
) -> Router {
    Router::new()
        .route("/push/{token}", get(accept).post(accept))
        .with_state(PushState {
            db,
            bus,
            clock,
            answer_time,
        })
}

/// Dropping the late answer rolls back its transaction, so a timed-out push records nothing.
async fn accept(
    State(state): State<PushState>,
    Path(token): Path<String>,
    query: Result<Query<PushQuery>, QueryRejection>,
    request: Request,
) -> Response {
    let limit = state.answer_time;
    let answer = answer(state, token, query, request);
    tokio::time::timeout(limit, answer)
        .await
        .unwrap_or_else(|_| StatusCode::SERVICE_UNAVAILABLE.into_response())
}

#[derive(Deserialize)]
struct PushQuery {
    status: Option<String>,
    msg: Option<String>,
    ping: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct Report {
    up: bool,
    message: Option<String>,
    ping_ms: Option<u32>,
}

impl PushQuery {
    /// The message is cleaned here and again when the store writes it (defence in depth).
    fn report(self) -> Option<Report> {
        let up = match self.status.as_deref() {
            None | Some("up") => true,
            Some("down") => false,
            Some(_) => return None,
        };
        let ping_ms = match self.ping {
            None => None,
            Some(text) => Some(text.parse::<u32>().ok().filter(|ms| *ms <= PING_MAX_MS)?),
        };
        let message = self
            .msg
            .map(|text| clean_message(&text))
            .filter(|text| !text.is_empty());
        Some(Report {
            up,
            message,
            ping_ms,
        })
    }
}

impl Report {
    fn into_probe(self) -> ProbeResult {
        ProbeResult {
            ok: self.up,
            response_time: self.ping_ms.map(|ms| Duration::from_millis(u64::from(ms))),
            message: self.message,
            resolved_ip: None,
            cert_not_after: None,
            domain_not_after: None,
            judgement: Judgement::Answered,
        }
    }
}

fn is_token_shaped(text: &str) -> bool {
    text.len() == TOKEN_LENGTH
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
}

/// Compares in constant time after the exact SQL match, as defence in depth.
fn accepts(check: &Check, token: &str) -> bool {
    let holds = check
        .push_token
        .as_ref()
        .is_some_and(|stored| bool::from(stored.expose().as_bytes().ct_eq(token.as_bytes())));
    holds && !check.paused
}

enum Outcome {
    Recorded,
    NotFound,
    Refused,
}

/// The store judges the attempt under the row lock; maintenance covers a push as it covers a scheduled attempt (CHK-006).
async fn store_beat(
    db: &PgPool,
    check: &Check,
    probe: &ProbeResult,
    now: DateTime<Utc>,
) -> Result<Option<Beat>, Error> {
    let device = devices::get(db, check.device)
        .await
        .map_err(Error::Devices)?;
    let in_maintenance = in_maintenance(db, &device, (check.id, check.parent), now)
        .await
        .map_err(Error::Maintenance)?;
    let attempt = Attempt {
        result: probe,
        in_maintenance,
        at: now,
        domain: None,
        device: &device,
    };
    let mut tx = db.begin().await?;
    let beat = store::record_attempt(&mut tx, check.id, &attempt).await?;
    tx.commit().await?;
    Ok(beat)
}

async fn record(
    state: &PushState,
    token: &str,
    query: Result<Query<PushQuery>, QueryRejection>,
) -> Result<Outcome, Error> {
    let Some(check) = store::by_push_token(&state.db, token).await? else {
        return Ok(Outcome::NotFound);
    };
    if !accepts(&check, token) {
        return Ok(Outcome::NotFound);
    }
    let Some(report) = query.ok().and_then(|Query(query)| query.report()) else {
        return Ok(Outcome::Refused);
    };
    let now = state.clock.now();
    report_in((&state.db, &state.bus), &check, report, now).await?;
    Ok(Outcome::Recorded)
}

async fn report_in(
    (db, bus): (&PgPool, &Bus),
    check: &Check,
    report: Report,
    now: DateTime<Utc>,
) -> Result<(), Error> {
    let Some(beat) = store_beat(db, check, &report.into_probe(), now).await? else {
        return Ok(());
    };
    bus.publish(Signal::Heartbeat {
        check: check.id,
        device: check.device,
        beat: HeartbeatSummary {
            at: beat.at,
            status: beat.status,
            response_time_ms: beat.response_time_ms,
            message: beat.message.as_deref().map(clean_message),
        },
    });
    Ok(())
}

async fn answer(
    state: PushState,
    token: String,
    query: Result<Query<PushQuery>, QueryRejection>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    // axum routes HEAD to a GET handler, and a probe of the URL is not the device reporting in.
    if parts.method == Method::HEAD {
        let allow = [(header::ALLOW, "GET, POST")];
        return (StatusCode::METHOD_NOT_ALLOWED, allow).into_response();
    }
    if !is_token_shaped(&token) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let declared = parts
        .headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok());
    if declared.is_some_and(|length| length > BODY_LIMIT) {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    // Reading the unused body under the cap is what refuses a larger one.
    if to_bytes(body, BODY_LIMIT).await.is_err() {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    match record(&state, &token, query).await {
        Ok(Outcome::Recorded) => (StatusCode::OK, "ok").into_response(),
        Ok(Outcome::Refused) => StatusCode::BAD_REQUEST.into_response(),
        Ok(Outcome::NotFound) | Err(Error::NotFound(_)) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            // The error's message holds no path, token or SQL (LOG-007).
            tracing::error!(%error, "a push result could not be recorded");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
