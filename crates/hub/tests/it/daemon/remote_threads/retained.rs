use super::*;

async fn not_created(hub: &Daemons, token: &str, work: &Value) {
    let evidence = json!({
        "device": PVE_01, "launch": LAUNCH, "work": work["id"], "attempt": 1,
        "outcome": "not_created", "cause": "stopped"
    });
    let (status, receipt) = post(hub, token, "/work/v1/thread-launch-evidence", evidence).await;
    assert_eq!(status, StatusCode::OK, "{receipt}");
    assert_eq!(receipt["state"], "stopped");
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn proven_non_creation_prevents_first_dispatch_even_with_a_live_lease(pool: PgPool) {
    let hub = Daemons::at(&pool, pushing_now(&pool).await).await;
    let token = hub.token_for(PVE_01).await;
    let (_, work) = admitted(&hub, &token).await;
    not_created(&hub, &token, &work).await;
    let replay = post(&hub, &token, "/work/v1/thread-dispatch", dispatch(&work)).await;
    assert_eq!(
        replay,
        (StatusCode::CONFLICT, json!({"error": "NOT_HOLDER"}))
    );
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn proven_non_creation_keeps_an_issued_grant_as_a_reconciliation_reference(pool: PgPool) {
    let hub = Daemons::at(&pool, pushing_now(&pool).await).await;
    let token = hub.token_for(PVE_01).await;
    let (_, work) = admitted(&hub, &token).await;
    let (status, grant) = post(&hub, &token, "/work/v1/thread-dispatch", dispatch(&work)).await;
    assert_eq!(status, StatusCode::OK, "{grant}");
    not_created(&hub, &token, &work).await;
    let replay = post(&hub, &token, "/work/v1/thread-dispatch", dispatch(&work)).await;
    assert_eq!(
        replay,
        (
            StatusCode::CONFLICT,
            json!({"error": "NOT_HOLDER", "authorization": grant["authorization"]})
        )
    );
}

const FOLLOW_UP: &str = "synthetic:follow-up";
const PROMPT: &str = "mutation($input: AgentThreadPromptInput!) {
    sendAgentThreadPrompt(input: $input) { command state }
}";

async fn active(hub: &Daemons, token: &str) -> (Value, Value) {
    let (receipt, work) = admitted(hub, token).await;
    assert_eq!(
        post(hub, token, "/work/v1/thread-dispatch", dispatch(&work))
            .await
            .0,
        StatusCode::OK
    );
    let now = anima_hub::clock::Clock::now(&*hub.clock);
    let mut batch = cpu_batch(PVE_01, now, 4.0);
    batch["agentThreads"] = json!({"count": 1, "countRevision": 1, "summaries": [{
        "runnerId": RUNNER, "program": "codex", "state": "idle", "title": "Synthetic",
        "startedAt": time(now), "activityAt": time(now), "revision": 1
    }]});
    assert_eq!(hub.push(token, &batch).await.0, StatusCode::OK);
    let evidence = json!({
        "device": PVE_01, "launch": LAUNCH, "work": work["id"], "attempt": 1,
        "outcome": "created", "thread": RUNNER, "cause": null
    });
    let (status, bound) = post(hub, token, "/work/v1/thread-launch-evidence", evidence).await;
    assert_eq!(status, StatusCode::OK, "{bound}");
    let control = json!({"device": PVE_01, "launch": LAUNCH, "runner": RUNNER,
        "offer": {"process": "process-1", "revision": 1, "actions": ["prompt", "answer"],
          "stopTargets": [], "requests": [{
              "kind": "approval", "id": "approval-1", "subject": "Check",
              "details": "Synthetic private approval details", "decisions": ["allow_once"], "rules": []
          }]
    }});
    assert_eq!(
        post(hub, token, "/work/v1/thread-control", control).await.0,
        StatusCode::NO_CONTENT
    );
    (
        receipt["data"]["startAgentThread"]["firstCommand"].clone(),
        bound["thread"].clone(),
    )
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../fixtures", scripts("devices"))
)]
async fn device_removal_scrubs_content_but_retains_receipts_and_conflicts(pool: PgPool) {
    let hub = Daemons::at(&pool, pushing_now(&pool).await).await;
    let token = hub.token_for(PVE_01).await;
    let (first, thread) = active(&hub, &token).await;
    let (status, delivery) =
        post(&hub, &token, "/work/v1/commands", json!({"device": PVE_01})).await;
    assert_eq!(status, StatusCode::OK, "{delivery}");
    assert_eq!(delivery["commands"][0]["id"], first);
    let prompt =
        json!({"input": {"command": FOLLOW_UP, "thread": thread, "prompt": "Synthetic follow-up"}});
    let waiting = hub.operate(PROMPT, prompt.clone()).await;
    assert_eq!(
        waiting["data"]["sendAgentThreadPrompt"]["state"], "waiting",
        "{waiting}"
    );

    change_source(&pool).await;
    let source_retry = hub.operate(START, start(LAUNCH)).await;
    assert_eq!(
        source_retry["data"]["startAgentThread"]["firstCommand"],
        first
    );
    assert_eq!(
        source_retry["data"]["startAgentThread"]["binding"],
        "unavailable"
    );
    assert_eq!(hub.operate(PROMPT, prompt.clone()).await, waiting);

    sqlx::query("DELETE FROM devices.devices WHERE id = $1::uuid")
        .bind(PVE_01)
        .execute(&pool)
        .await
        .unwrap();
    assert_scrubbed(&pool).await;
    retained_retries(&hub, &first, prompt).await;
}

async fn retained_retries(hub: &Daemons, first: &Value, prompt: Value) {
    let deleted_retry = hub.operate(START, start(LAUNCH)).await;
    assert_eq!(
        deleted_retry["data"]["startAgentThread"]["firstCommand"],
        *first
    );
    assert_eq!(
        deleted_retry["data"]["startAgentThread"]["thread"],
        Value::Null
    );
    let mut changed = start(LAUNCH);
    changed["input"]["prompt"] = json!("Different input");
    assert_eq!(
        hub.operate(START, changed).await["errors"][0]["extensions"]["code"],
        "LAUNCH_CONFLICT"
    );

    let retry = hub.operate(PROMPT, prompt.clone()).await;
    assert_eq!(retry["data"]["sendAgentThreadPrompt"]["state"], "expired");
    let mut changed_prompt = prompt;
    changed_prompt["input"]["prompt"] = json!("Different follow-up");
    assert_eq!(
        hub.operate(PROMPT, changed_prompt).await["errors"][0]["extensions"]["code"],
        "COMMAND_CONFLICT"
    );
    let outcome = hub
        .operate(
            "query($id: ID!) { agentThreadCommand(command: $id) { state } }",
            json!({"id": first}),
        )
        .await;
    assert_eq!(outcome["data"]["agentThreadCommand"]["state"], "delivered");
}

async fn change_source(pool: &PgPool) {
    sqlx::query("UPDATE devices.devices SET metrics_source = 'ssh' WHERE id = $1::uuid")
        .bind(PVE_01)
        .execute(pool)
        .await
        .unwrap();
    let (revision, hidden): (i64, bool) =
        sqlx::query_as("SELECT revision, offer IS NULL FROM agent_threads.control_offers WHERE launch_id = $1::uuid")
            .bind(LAUNCH).fetch_one(pool).await.unwrap();
    assert_eq!((revision, hidden), (1, true));
}

async fn assert_scrubbed(pool: &PgPool) {
    let (offers, bodies): (i64, i64) = sqlx::query_as(
        "SELECT
            (SELECT count(*) FROM agent_threads.control_offers WHERE launch_id = $1::uuid),
            (SELECT count(*) FROM agent_threads.commands WHERE launch_id = $1::uuid AND payload IS NOT NULL)"
    ).bind(LAUNCH).fetch_one(pool).await.unwrap();
    assert_eq!((offers, bodies), (0, 0));
}
