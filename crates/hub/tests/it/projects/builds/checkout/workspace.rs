//! The enrolled workspace route releases a GitHub credential only for one live offer.
use std::{sync::Arc, time::Duration};

use anima_hub::work::checkout;
use axum::{
    body::Body,
    http::{Request, StatusCode, header},
    response::Response,
};
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;
use tokio::sync::Semaphore;
use uuid::Uuid;

use super::fixture::{CHECKOUT, Fixture, MINT, PVE_01, json_body, send};

const QUEUE: &str = "mutation($device:ID!, $input:ManagedWorkspaceInput!){
    queueManagedWorkspaceRequest(device:$device,input:$input)}";
const SECRET: &str = "/daemon/v3/workspaces/checkout";

fn request(path: &str, enrollment: &str, payload: &Value) -> Request<Body> {
    Request::post(path)
        .header(header::AUTHORIZATION, format!("Bearer {enrollment}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap()
}

async fn poll(fixture: &Fixture, version: u8, report: Value) -> Value {
    let route = format!("/daemon/v{version}/workspaces/poll");
    let body = json!({"device":PVE_01,"ready":true,"report":report,"capacity":{
        "storageBytes":8_u64<<30,"memoryBytes":2_u64<<30,"cpuMillis":1000,"tasks":256
    }});
    let response = send(&fixture.app, request(&route, &fixture.primary, &body)).await;
    let status = response.status();
    let value = json_body(response).await;
    assert_eq!(status, StatusCode::OK, "{value}");
    value
}

async fn queue(fixture: &Fixture, workspace: Uuid, action: Value) -> (Uuid, Value) {
    let id = Uuid::now_v7();
    let answer = fixture
        .kit
        .post(
            QUEUE,
            json!({"device":PVE_01,"input":{
                "request":id,"workspace":workspace,"action":action
            }}),
        )
        .await;
    (id, answer)
}

async fn prepare(fixture: &Fixture) -> Uuid {
    let workspace = Uuid::now_v7();
    let source = json!({"owner":"me","repository":"private","commit":"ab".repeat(20),
        "repository_id":fixture.repository});
    let prepare = json!({"kind":"prepare","storage_bytes":1_u64<<30,"source":source});
    poll(fixture, 2, Value::Null).await;
    let (_, denied) = queue(fixture, workspace, prepare.clone()).await;
    assert_eq!(
        denied["errors"][0]["extensions"]["code"], "CONFLICT",
        "{denied}"
    );
    poll(fixture, 3, Value::Null).await;
    let (prep, accepted) = queue(fixture, workspace, prepare).await;
    assert_eq!(
        accepted["data"]["queueManagedWorkspaceRequest"],
        prep.to_string(),
        "{accepted}"
    );
    assert!(poll(fixture, 2, Value::Null).await["command"].is_null());
    let prepared = poll(fixture, 3, Value::Null).await;
    assert_eq!(prepared["command"]["request"], prep.to_string());
    assert!(prepared["checkout_repository"].is_null());
    let prepared_receipt = json!({"command":prepared["command"],"result":{
        "workspace":workspace,"generation":0,"phase":"prepared","storageBytes":1_u64<<30,
        "checkout":{"source":source,"ready":false}
    },"failure":null});
    poll(fixture, 3, prepared_receipt).await;
    workspace
}

async fn offer(fixture: &Fixture) -> (Uuid, Value, Value) {
    let workspace = prepare(fixture).await;
    fixture.clock.advance(TimeDelta::seconds(1));
    let (start, accepted) = queue(
        fixture,
        workspace,
        json!({"kind":"start","generation":1,
        "memory_bytes":1_u64<<30,"cpu_millis":500,"tasks":128}),
    )
    .await;
    assert_eq!(
        accepted["data"]["queueManagedWorkspaceRequest"],
        start.to_string(),
        "{accepted}"
    );
    let fetch = json!({"request":start,"repository_id":fixture.repository});
    assert_eq!(
        secret(fixture, &fixture.primary, &fetch).await.status(),
        StatusCode::CONFLICT
    );
    let offered = poll(fixture, 3, Value::Null).await;
    assert_eq!(offered["command"]["request"], start.to_string());
    assert_eq!(offered["checkout_repository"], fixture.repository);
    (start, offered["command"].clone(), fetch)
}

async fn secret(fixture: &Fixture, token: &str, payload: &Value) -> Response {
    send(&fixture.app, request(SECRET, token, payload)).await
}

async fn release_audits(pool: &PgPool, start: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM events.audit_log
        WHERE action='fetch' AND change->>'purpose'='workspace_checkout' AND change->>'workspaceRequest'=$1")
        .bind(start.to_string()).fetch_one(pool).await.unwrap()
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../fixtures", scripts("devices"))
)]
async fn checkout_releases_once_for_the_offered_start_then_revokes_after_receipt(pool: PgPool) {
    let fixture = Fixture::new(&pool).await;
    let (start, command, fetch) = offer(&fixture).await;
    assert_eq!(
        secret(&fixture, &fixture.secondary, &fetch).await.status(),
        StatusCode::CONFLICT
    );
    let wrong = json!({"request":start,"repository_id":Uuid::now_v7()});
    assert_eq!(
        secret(&fixture, &fixture.primary, &wrong).await.status(),
        StatusCode::CONFLICT
    );
    let before = fixture.kit.stub.paths().len();
    fixture.mint_reply();
    let response = secret(&fixture, &fixture.primary, &fetch).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(json_body(response).await["value"], CHECKOUT);
    assert_eq!(
        secret(&fixture, &fixture.primary, &fetch).await.status(),
        StatusCode::CONFLICT
    );
    assert_eq!(release_audits(&pool, start).await, 1);
    checkout::revoke_due(&fixture.scheduling).await.unwrap();
    assert_eq!(&fixture.kit.stub.paths()[before..], [MINT]);
    poll(
        &fixture,
        3,
        json!({"command":command,"result":null,"failure":"Workspace start failed."}),
    )
    .await;
    checkout::revoke_due(&fixture.scheduling).await.unwrap();
    fixture.assert_revoked(before);
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../fixtures", scripts("devices"))
)]
async fn an_expired_offer_is_revoked_even_after_the_checkout_http_deadline(pool: PgPool) {
    let fixture = Fixture::new(&pool).await;
    let (start, _, fetch) = offer(&fixture).await;
    let before = fixture.kit.stub.paths().len();
    fixture.mint_reply();
    let mint = Arc::new(Semaphore::new(0));
    fixture.kit.stub.gate(MINT, &mint);
    let (app, request) = (
        fixture.app.clone(),
        request(SECRET, &fixture.primary, &fetch),
    );
    let pending = tokio::spawn(async move { send(&app, request).await });
    tokio::time::timeout(Duration::from_secs(4), async {
        while fixture.kit.stub.paths().len() == before {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();

    let mut lock = pool.begin().await.unwrap();
    let lock_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *lock)
        .await
        .unwrap();
    sqlx::query("SELECT device_id FROM metrics.daemons WHERE device_id=$1 FOR UPDATE")
        .bind(PVE_01.parse::<Uuid>().unwrap())
        .execute(&mut *lock)
        .await
        .unwrap();
    fixture.clock.advance(TimeDelta::seconds(16));
    mint.add_permits(1);
    wait_for_enrolment_check(&pool, lock_pid).await;
    let reply = pending.await.unwrap();
    assert_eq!(reply.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(json_body(reply).await["value"].is_null());
    fixture.clock.advance(TimeDelta::seconds(16));
    checkout::revoke_due(&fixture.scheduling).await.unwrap();
    fixture.assert_revoked(before);
    lock.rollback().await.unwrap();
    assert_eq!(release_audits(&pool, start).await, 0);
}

async fn wait_for_enrolment_check(pool: &PgPool, lock_pid: i32) {
    tokio::time::timeout(Duration::from_secs(4), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "SELECT EXISTS(
                SELECT 1 FROM pg_stat_activity WHERE datname=current_database()
                  AND $1=ANY(pg_blocking_pids(pid)))",
            )
            .bind(lock_pid)
            .fetch_one(pool)
            .await
            .unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the minted credential must reach the final enrolment check before the HTTP deadline");
}
