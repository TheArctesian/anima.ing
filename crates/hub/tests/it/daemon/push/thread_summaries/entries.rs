use std::time::Duration;

use anima_hub::{
    fleet::{
        access::Identity,
        events::{Bus, Operator},
    },
    ids::OperatorId,
    schema,
};
use async_graphql::{Request as GraphRequest, futures_util::StreamExt as _};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::summaries;
use crate::daemon::{
    batches::time,
    harness::{Daemons, PERMITTED, PVE_01, pushing_now},
};

async fn upload(hub: &Daemons, token: &str, summary: &Value, sequence: i64) -> (StatusCode, Value) {
    let body = json!({
        "device": PVE_01,
        "summary": summary,
        "entries": [{ "sequence": sequence, "content": { "change": "prompt", "text": format!("step {sequence}") } }],
    });
    let request = Request::post("/daemon/v1/threads/entries")
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, body) = hub.send(request, PERMITTED).await;
    (status, serde_json::from_str(&body).unwrap())
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../fixtures", scripts("devices"))
)]
async fn thread_handoff_replays_a_commit_between_snapshot_and_resume(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let hub = Daemons::at(&pool, now).await;
    let token = hub.token_for(PVE_01).await;
    let summary = &summaries(&time(now), 1, 1, "Test thread")["summaries"][0];
    let (status, receipt) = upload(&hub, &token, summary, 1).await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    let id = receipt["id"].as_str().unwrap();
    let snapshot = hub
        .operate(
            "query($id: ID!) { agentThread(id: $id) { lastSequence entries { sequence } } }",
            json!({"id": id}),
        )
        .await;
    assert_eq!(
        snapshot["data"]["agentThread"],
        json!({
            "lastSequence": "1", "entries": [{ "sequence": "1" }],
        })
    );
    assert_eq!(
        upload(&hub, &token, summary, 3).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(upload(&hub, &token, summary, 2).await.0, StatusCode::OK);
    let repeated = upload(&hub, &token, summary, 2).await;
    assert_eq!(
        repeated,
        (StatusCode::OK, json!({"id": id, "lastSequence": 2}))
    );

    let schema = schema::build_schema(crate::hub::dependencies(
        &pool,
        hub.clock.clone(),
        Bus::new(64),
    ));
    let operator = Operator {
        id: "0190a000-0000-7000-8000-000000000001"
            .parse::<OperatorId>()
            .unwrap(),
        name: "operator".into(),
        actor: anima_hub::fleet::events::Actor::for_tests(),
    };
    let request = Identity::for_tests(operator, now + TimeDelta::days(1)).attach(GraphRequest::new(
        format!(r#"subscription {{ agentThreadEntries(id: "{id}", afterSequence: "1") {{ sequence content }} }}"#),
    ));
    let mut stream = Box::pin(schema.execute_stream(request));
    let replay = tokio::time::timeout(Duration::from_secs(3), stream.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        replay.data.into_json().unwrap()["agentThreadEntries"]["sequence"],
        "2"
    );

    assert_eq!(upload(&hub, &token, summary, 3).await.0, StatusCode::OK);
    let live = tokio::time::timeout(Duration::from_secs(3), stream.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        live.data.into_json().unwrap()["agentThreadEntries"]["sequence"],
        "3"
    );
}
