use anima_hub::{fleet::devices, work::jobs};
use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use super::{Daemons, LAUNCH, PVE_01, START, offer, post, pushing_now, start};

async fn set_work(pool: &PgPool, on: bool) {
    let now = pushing_now(pool).await;
    let mut tx = pool.begin().await.unwrap();
    let device = PVE_01.parse().unwrap();
    devices::lock(&mut tx, device).await.unwrap();
    jobs::set_device_work(&mut tx, device, on, now)
        .await
        .unwrap();
    tx.commit().await.unwrap();
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn a_blocked_mixed_offer_waits_then_delivers_model_reduction_before_thread(pool: PgPool) {
    let hub = Daemons::at(&pool, pushing_now(&pool).await).await;
    let token = hub.token_for(PVE_01).await;
    let mut claim = offer();
    claim["types"] = json!(["agent_thread", "model_drain"]);
    claim["kinds"] = json!(["person", "model"]);
    let route = "/work/v1/claim";
    assert_eq!(
        post(&hub, &token, route, claim.clone()).await.0,
        StatusCode::NO_CONTENT
    );
    let reply = hub.operate(START, start(LAUNCH)).await;
    assert_eq!(
        reply["data"]["startAgentThread"]["state"], "queued",
        "{reply}"
    );
    sqlx::query(
        "INSERT INTO jobs.jobs (id, work_type, kind, actor, parameters, machine_id,
            state, created_at, state_changed_at, limit_seconds, limit_idle_seconds,
            limit_memory_bytes, limit_disk_bytes, limit_orphan_seconds)
         VALUES (gen_random_uuid(), 'model_drain', 'model', 'Hub', '{\"seconds\":60}', $1::uuid,
            'queued', clock_timestamp(), clock_timestamp(), 60, 1, 0, 0, 1)",
    )
    .bind(PVE_01)
    .execute(&pool)
    .await
    .unwrap();

    set_work(&pool, false).await;
    assert_eq!(
        post(&hub, &token, route, claim.clone()).await.0,
        StatusCode::NO_CONTENT
    );
    set_work(&pool, true).await;
    let (status, answer) = post(&hub, &token, route, claim.clone()).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["type"], "model_drain");
    claim["holds"] = json!([{"id": answer["id"], "attempt": answer["attempt"]}]);
    let (status, answer) = post(&hub, &token, route, claim).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["type"], "agent_thread");
    assert_eq!(answer["parameters"]["launch"], LAUNCH);
}
