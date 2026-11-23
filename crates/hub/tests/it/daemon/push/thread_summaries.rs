use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::daemon::{
    batches::{cpu_batch, samples_of, time},
    harness::{DOCKER_HOST, Daemons, PVE_01, pushing_now, refusal},
};

const RUNNER_ID: &str = "1a462eb7-fd11-4531-acad-9942cfc1f586";

fn summaries(at: &str, revision: i64, count: u32, title: &str) -> Value {
    json!({
        "count": count,
        "countRevision": revision,
        "summaries": [{
            "runnerId": RUNNER_ID,
            "program": "codex",
            "state": "needs_you",
            "title": title,
            "startedAt": at,
            "activityAt": at,
            "revision": revision,
        }],
    })
}

async fn receipt(daemons: &Daemons, token: &str, batch: &Value) -> Value {
    let (status, body) = daemons.push(token, batch).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    serde_json::from_str::<Value>(&body).unwrap()["agentThreads"].clone()
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn thread_summaries_acknowledge_retries_without_rolling_back_state(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let token = daemons.token_for(PVE_01).await;
    let mut batch = cpu_batch(PVE_01, now, 5.0);
    let legacy = batch.clone();
    assert_eq!(
        daemons.push(&token, &legacy).await.0,
        StatusCode::NO_CONTENT
    );

    batch["agentThreads"] = summaries(&time(now), 1, 1, "First title");
    let first = receipt(&daemons, &token, &batch).await;
    let id = first["summaries"][0]["id"].as_str().unwrap();
    let first_batch = batch.clone();
    batch["agentThreads"] = summaries(&time(now), 2, 2, "Changed title");
    let updated = receipt(&daemons, &token, &batch).await;
    assert_eq!(updated["countRevision"], 2);
    assert_eq!(
        updated["summaries"],
        json!([{ "runnerId": RUNNER_ID, "id": id, "revision": 2 }])
    );
    assert_eq!(receipt(&daemons, &token, &batch).await, updated);
    assert_eq!(receipt(&daemons, &token, &first_batch).await, updated);
    assert_eq!(
        daemons.push(&token, &legacy).await.0,
        StatusCode::NO_CONTENT
    );

    let stored: (String, i64) = sqlx::query_as(
        "SELECT s.title, c.count FROM agent_threads.summaries s
         JOIN agent_threads.counts c USING (device_id) WHERE s.id = $1::uuid",
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(stored, ("Changed title".into(), 2));
    assert_eq!(samples_of(&pool, PVE_01).await, 1);
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn thread_summaries_are_scoped_to_the_authenticated_device(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let first_token = daemons.token_for(PVE_01).await;
    let second_token = daemons.token_for(DOCKER_HOST).await;
    let mut first = cpu_batch(PVE_01, now, 5.0);
    let mut second = cpu_batch(DOCKER_HOST, now, 5.0);
    first["agentThreads"] = summaries(&time(now), 1, 1, "First device");
    second["agentThreads"] = summaries(&time(now), 1, 1, "Second device");

    assert_eq!(
        daemons.push(&first_token, &second).await,
        (StatusCode::FORBIDDEN, refusal("WRONG_DEVICE"))
    );
    let first = receipt(&daemons, &first_token, &first).await;
    let other = receipt(&daemons, &second_token, &second).await;
    assert_ne!(first["summaries"][0]["id"], other["summaries"][0]["id"]);
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn malformed_thread_summary_is_refused_with_the_whole_batch(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let token = daemons.token_for(PVE_01).await;
    let mut batch = cpu_batch(PVE_01, now, 5.0);
    batch["agentThreads"] = summaries(&time(now), 1, 1, "Bad\nTitle");

    assert_eq!(
        daemons.push(&token, &batch).await,
        (StatusCode::BAD_REQUEST, refusal("MALFORMED"))
    );
    assert_eq!(samples_of(&pool, PVE_01).await, 0);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_threads.counts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

mod entries;
