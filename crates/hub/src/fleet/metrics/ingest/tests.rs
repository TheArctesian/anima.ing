mod answering;
mod automatic_access;
mod credential;
mod enrolment;
mod managed_polls;
mod refusals;
mod renewal;
mod stale;
mod throttle;
mod turns;

use std::sync::Arc;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{DateTime, DurationRound as _, TimeDelta, Utc};
use daemon_core::wire::{DaemonToken, EnrolmentCode};
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt as _;

use super::{
    enrolment::{enrol, make_code, redeem},
    routes::{Deadlines, daemon_router_within},
};
use crate::{
    clock::{Clock, FixedClock},
    fleet::events::{Bus, ClientAddr},
    ids::DeviceId,
};

/// pve-01 of the devices fixture.
fn pve() -> DeviceId {
    "0190a000-0000-7000-8002-000000000001".parse().unwrap()
}

async fn database_now(pool: &PgPool) -> DateTime<Utc> {
    let now: DateTime<Utc> = sqlx::query_scalar("SELECT pg_catalog.now()")
        .fetch_one(pool)
        .await
        .unwrap();
    now.duration_trunc(TimeDelta::seconds(1)).unwrap()
}

async fn push(pool: &PgPool, now: DateTime<Utc>, token: &DaemonToken, batch: &Value) -> StatusCode {
    let clock: Arc<dyn Clock> = Arc::new(FixedClock(now));
    let router = daemon_router_within(pool.clone(), Bus::new(16), clock, Deadlines::HUB);
    let mut request = Request::post("/daemon/v1/push")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {}", token.expose()))
        .body(Body::from(batch.to_string()))
        .unwrap();
    let client = ClientAddr("100.64.0.7".parse().unwrap());
    request.extensions_mut().insert(client);
    router.oneshot(request).await.unwrap().status()
}

async fn enrolled_pve(pool: &PgPool, now: DateTime<Utc>, seed: u8) -> DaemonToken {
    enrolled(pool, pve(), now, seed).await
}

async fn enrolled(pool: &PgPool, device: DeviceId, now: DateTime<Utc>, seed: u8) -> DaemonToken {
    let code = EnrolmentCode::from_random([seed; 16]);
    let presented = EnrolmentCode::parse(code.expose()).unwrap();
    let mut conn = pool.acquire().await.unwrap();
    make_code(&mut conn, device, code, now).await.unwrap();
    drop(conn);
    let redeemed = redeem(pool, &presented, now).await.unwrap().unwrap();
    let token = DaemonToken::from_random([seed; 32]);
    let kept = DaemonToken::parse(token.expose()).unwrap();
    let enrolled = enrol(pool, &Bus::new(4), (redeemed, token), now)
        .await
        .unwrap();
    assert!(enrolled.is_some());
    kept
}

async fn token_hashes(pool: &PgPool) -> (Vec<u8>, Option<Vec<u8>>) {
    sqlx::query_as(
        "SELECT token_sha256, next_token_sha256 FROM metrics.daemons WHERE device_id = $1",
    )
    .bind(pve())
    .fetch_one(pool)
    .await
    .unwrap()
}
