use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{
    batches::{cpu_batch, time},
    harness::{Daemons, PERMITTED, PVE_01, pushing_now},
};

const LAUNCH: &str = "00000000-0000-4000-8000-000000000010";
const PROJECT: &str = "00000000-0000-4000-8000-000000000030";
const RUNNER: &str = "00000000-0000-4000-8000-000000000020";
const START: &str = "mutation($input: AgentThreadStartInput!) {
    startAgentThread(input: $input) { launch state binding thread firstCommand }
}";

async fn post(hub: &Daemons, token: &str, route: &str, body: Value) -> (StatusCode, Value) {
    let request = Request::post(route)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap();
    let (status, body) = hub.send(request, PERMITTED).await;
    let body = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&body).unwrap()
    };
    (status, body)
}

fn offer() -> Value {
    json!({
        "device": PVE_01, "switch": "off", "threads": "on",
        "types": ["agent_thread"], "kinds": ["person"], "os": "linux", "architecture": "x86_64",
        "free": {"slots": 1, "memoryBytes": 0, "diskBytes": 0},
        "threadTargets": [{"project": PROJECT, "folder": "/var/code/example", "program": "codex"}]
    })
}

fn start(launch: &str) -> Value {
    json!({"input": {
        "launch": launch, "device": PVE_01, "project": PROJECT,
        "folder": "/var/code/example", "program": "codex", "access": "ask",
        "prompt": "Synthetic first prompt"
    }})
}

async fn admitted(hub: &Daemons, token: &str) -> (Value, Value) {
    assert_eq!(
        post(hub, token, "/work/v1/claim", offer()).await.0,
        StatusCode::NO_CONTENT
    );
    let reply = hub.operate(START, start(LAUNCH)).await;
    assert_eq!(
        reply["data"]["startAgentThread"]["state"], "queued",
        "{reply}"
    );
    let (status, work) = post(hub, token, "/work/v1/claim", offer()).await;
    assert_eq!(status, StatusCode::OK, "{work}");
    assert_eq!(work["type"], "agent_thread");
    assert_eq!(work["project"], PROJECT);
    assert!(work.get("secrets").is_none());
    assert!(work.get("repository").is_none());
    assert!(!work.to_string().contains("Synthetic first prompt"));
    (reply, work)
}

fn dispatch(work: &Value) -> Value {
    json!({"device": PVE_01, "launch": LAUNCH, "work": work["id"], "attempt": 1, "runner": RUNNER})
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn work_off_wins_before_dispatch_and_exact_non_creation_reconciles(pool: PgPool) {
    let hub = Daemons::at(&pool, pushing_now(&pool).await).await;
    let token = hub.token_for(PVE_01).await;
    let (receipt, work) = admitted(&hub, &token).await;
    let mut tx = pool.begin().await.unwrap();
    let device = PVE_01.parse().unwrap();
    anima_hub::fleet::devices::lock(&mut tx, device)
        .await
        .unwrap();
    anima_hub::work::jobs::set_device_work(&mut tx, device, false, pushing_now(&pool).await)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let (status, denied) = post(&hub, &token, "/work/v1/thread-dispatch", dispatch(&work)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(denied["error"], "WORK_OFF");
    let evidence = json!({
        "device": PVE_01, "launch": LAUNCH, "work": work["id"], "attempt": 1,
        "outcome": "not_created", "cause": "refused"
    });
    let (status, refused) = post(
        &hub,
        &token,
        "/work/v1/thread-launch-evidence",
        evidence.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{refused}");
    assert_eq!(refused["state"], "refused");
    assert_eq!(refused["binding"], "unavailable");
    assert_eq!(
        post(&hub, &token, "/work/v1/thread-launch-evidence", evidence)
            .await
            .1,
        refused
    );
    let first = receipt["data"]["startAgentThread"]["firstCommand"].clone();
    let command = hub
        .operate(
            "query($id: ID!) { agentThreadCommand(command: $id) { state cause } }",
            json!({"id": first}),
        )
        .await;
    assert_eq!(command["data"]["agentThreadCommand"]["state"], "refused");
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn exact_retry_survives_capacity_and_summary_alone_never_binds(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let hub = Daemons::at(&pool, now).await;
    let token = hub.token_for(PVE_01).await;
    let other = hub.token_for(super::harness::DOCKER_HOST).await;
    let (original, work) = admitted(&hub, &token).await;
    fill(&hub).await;
    let retry = hub.operate(START, start(LAUNCH)).await;
    assert_eq!(
        retry["data"]["startAgentThread"]["firstCommand"],
        original["data"]["startAgentThread"]["firstCommand"]
    );
    let mut changed = start(LAUNCH);
    changed["input"]["prompt"] = json!("Different input");
    assert_eq!(
        hub.operate(START, changed).await["errors"][0]["extensions"]["code"],
        "LAUNCH_CONFLICT"
    );

    let (status, grant) = post(&hub, &token, "/work/v1/thread-dispatch", dispatch(&work)).await;
    assert_eq!(status, StatusCode::OK, "{grant}");
    let owner: uuid::Uuid = sqlx::query_scalar("SELECT id FROM access.operators")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(work["parameters"]["account"], owner.to_string());

    sqlx::query(
        "UPDATE agent_threads.launches SET issued_tick = issued_tick - 11000000000
                 WHERE launch_id = $1::uuid",
    )
    .bind(LAUNCH)
    .execute(&pool)
    .await
    .unwrap();
    let (status, expired) = post(&hub, &token, "/work/v1/thread-dispatch", dispatch(&work)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        expired,
        json!({"error": "NOT_HOLDER", "authorization": grant["authorization"]})
    );

    let mut batch = cpu_batch(PVE_01, now, 4.0);
    batch["agentThreads"] = json!({"count": 1, "countRevision": 1, "summaries": [{
        "runnerId": RUNNER, "program": "codex", "state": "idle", "title": "Synthetic",
        "startedAt": time(now), "activityAt": time(now), "revision": 1
    }]});
    let (status, summary) = hub.push(&token, &batch).await;
    assert_eq!(status, StatusCode::OK, "{summary}");
    let summary: Value = serde_json::from_str(&summary).unwrap();
    let thread = &summary["agentThreads"]["summaries"][0]["id"];
    assert_eq!(
        reopen(&hub, thread).await["data"]["agentThreadControl"]["offer"],
        Value::Null
    );

    let mut evidence = dispatch(&work);
    evidence.as_object_mut().unwrap().remove("runner");
    evidence["outcome"] = json!("created");
    evidence["thread"] = json!(RUNNER);
    assert_eq!(
        post(
            &hub,
            &other,
            "/work/v1/thread-launch-evidence",
            evidence.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, confirmed) = post(&hub, &token, "/work/v1/thread-launch-evidence", evidence).await;
    assert_eq!(status, StatusCode::OK, "{confirmed}");
    assert_eq!(confirmed["state"], "started");
    assert_eq!(confirmed["thread"], *thread);
    settle_first(&hub, &token, (&original, thread), owner).await;
}

async fn settle_first(
    hub: &Daemons,
    token: &str,
    (original, thread): (&Value, &Value),
    owner: uuid::Uuid,
) {
    let control = json!({"device": PVE_01, "launch": LAUNCH, "runner": RUNNER,
        "offer": {"process": "process-1", "revision": 7, "actions": ["prompt"],
                  "stopTargets": [], "requests": []}});
    assert_eq!(
        post(hub, token, "/work/v1/thread-control", control).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        reopen(hub, thread).await["data"]["agentThreadControl"]["offer"]["revision"].as_f64(),
        Some(7.0)
    );
    let first = &original["data"]["startAgentThread"]["firstCommand"];
    let (status, delivered) =
        post(hub, token, "/work/v1/commands", json!({"device": PVE_01})).await;
    assert_eq!(status, StatusCode::OK, "{delivered}");
    assert_eq!(
        delivered["commands"],
        json!([{
            "id": first, "account": owner, "device": PVE_01, "launch": LAUNCH, "thread": RUNNER,
            "payload": {"kind": "prompt", "prompt": "Synthetic first prompt"}
        }])
    );
    let receipt = json!({"id": first, "outcome": "recorded", "cause": null});
    assert_eq!(
        post(hub, token, "/work/v1/command-result", receipt).await.0,
        StatusCode::OK
    );
    let settled = hub
        .operate(
            "query($id: ID!) { agentThreadCommand(command: $id) { state cause } }",
            json!({"id": first}),
        )
        .await;
    assert_eq!(
        settled["data"]["agentThreadCommand"],
        json!({"state": "recorded", "cause": null})
    );
}

async fn fill(hub: &Daemons) {
    for index in 1..=31 {
        let id = format!("00000000-0000-4000-8001-{index:012}");
        let result = hub.operate(START, start(&id)).await;
        if index == 31 {
            assert_eq!(result["errors"][0]["extensions"]["code"], "COMMANDS_FULL");
        } else {
            assert_eq!(
                result["data"]["startAgentThread"]["state"], "queued",
                "{result}"
            );
        }
    }
}

async fn reopen(hub: &Daemons, id: &Value) -> Value {
    hub.operate(
        "query($id: ID!) { agentThreadControl(thread: $id) { offer { revision } } }",
        json!({"id": id}),
    )
    .await
}

mod arbitration;
mod retained;
