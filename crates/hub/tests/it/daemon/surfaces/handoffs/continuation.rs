use super::{
    Body, Fixture, Method, Offer, PERMITTED, PgPool, StatusCode, call, contents,
    placement::{OTHER, capacity, enabled, poll_source},
    report, send, transfer_state,
};
use agent_threads::managed::workspace::{Capacity, handoff::Source};
use anima_hub::clock::Clock;
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use uuid::Uuid;

const MEMORY: u64 = 1 << 30;
const CPU: u64 = 750;
const TASKS: u64 = 128;

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../fixtures", scripts("devices"))
)]
async fn a_move_continues_once_after_capacity_and_destination_sign_in(pool: PgPool) {
    let fixture = Fixture::with_continuation(&pool).await;
    await_named_export(&fixture).await;
    let import = super::withdrawal::activation_offer(&fixture).await;
    assert_eq!(
        report(&fixture, "activated", &import, &fixture.destination).await,
        StatusCode::NO_CONTENT
    );
    let (launch, poll) = start_workspace(&fixture).await;
    let source = fixture.receipt.source;
    let view = code_workspaces(&fixture).await;
    assert_eq!(view["provider"], "codex");
    assert_eq!(view["signedIn"], false);
    assert!(view["sourceThread"].is_string());
    assert!(post(&fixture, "/daemon/v1/code/poll", &poll).await["command"].is_null());

    // The real sign-in subscription records this fact after the authenticated provider reply.
    sqlx::query(
        "UPDATE agent_threads.workspace_continuations SET provider_ready_at=$2 WHERE transfer=$1",
    )
    .bind(source.binding.transfer)
    .bind(fixture.daemons.clock.now())
    .execute(&pool)
    .await
    .unwrap();
    let delivery = post(&fixture, "/daemon/v1/code/poll", &poll).await;
    let command = &delivery["command"];
    assert_eq!(command["request"], view["request"]);
    assert_eq!(command["principal"], source.principal.to_string());
    assert_eq!(command["workspace"], source.workspace.to_string());
    assert_eq!(command["generation"], launch["action"]["generation"]);
    assert_eq!(command["action"]["program"], "codex");
    assert_eq!(command["action"]["mode"], "ask");
    let prompt = command["action"]["prompt"].as_str().unwrap();
    assert!(prompt.contains("Update the homepage"));
    assert!(prompt.contains("Add a footer"));
    assert!(!prompt.contains("private tool output"));
    let mut acknowledged = poll;
    acknowledged["reports"] = json!([{
        "request":command["request"], "workspace":source.workspace, "principal":source.principal,
        "generation":command["generation"],
        "result":{"outcome":"started", "thread":command["request"], "folder":"/home/agent/workspace"}
    }]);
    let done = post(&fixture, "/daemon/v1/code/poll", &acknowledged).await;
    assert_eq!(done["acknowledged"][0], command["request"]);
    assert!(done["command"].is_null());
    assert_eq!(code_workspaces(&fixture).await["delivery"], "completed");
}

async fn await_named_export(fixture: &Fixture) {
    let source = fixture.receipt.source;
    let destination = source.binding.destination;
    let other = fixture.daemons.token_for(OTHER).await;
    capacity(fixture, &other, OTHER.parse().unwrap(), TASKS * 2).await;
    capacity(fixture, &fixture.destination, destination, TASKS - 1).await;
    assert_eq!(poll_source(fixture).await.status(), StatusCode::NO_CONTENT);
    let waiting = transfer_state(fixture).await;
    assert_eq!(waiting["state"], "waiting-placement");
    assert_eq!(waiting["destinationDevice"], destination.to_string());
    assert!(enabled(fixture).await);
    verify_named_retry(fixture, source).await;
    claim_named_export(fixture, source).await;
}

async fn claim_named_export(fixture: &Fixture, source: Source) {
    capacity(
        fixture,
        &fixture.destination,
        source.binding.destination,
        TASKS,
    )
    .await;
    let response = poll_source(fixture).await;
    assert_eq!(response.status(), StatusCode::OK);
    let offer: Offer = serde_json::from_slice(&contents(response).await).unwrap();
    assert!(matches!(offer, Offer::Export { source: planned, .. } if planned == source));
    assert!(!enabled(fixture).await);
}

async fn verify_named_retry(fixture: &Fixture, source: Source) {
    use super::fixture::{named_action, submit};

    let action = named_action(source, true);
    let replay = submit(&fixture.app, &fixture.daemons, source, action.clone()).await;
    assert_eq!(
        replay["data"]["queueManagedWorkspaceRequest"],
        source.binding.transfer.to_string()
    );
    let mut changed = action;
    changed["destination_device"] = json!(OTHER);
    let refused = submit(&fixture.app, &fixture.daemons, source, changed).await;
    assert_eq!(refused["errors"][0]["extensions"]["code"], "CONFLICT");
}

async fn start_workspace(fixture: &Fixture) -> (Value, Value) {
    let source = fixture.receipt.source;
    let mut poll = json!({"device":source.binding.destination,"ready":true,"report":null,
        "capacity":Capacity {memory_bytes: MEMORY - 1, ..Capacity::MAXIMUM}});
    assert!(post(fixture, "/daemon/v2/workspaces/poll", &poll).await["command"].is_null());
    poll["capacity"]["memoryBytes"] = json!(MEMORY);
    let launched = post(fixture, "/daemon/v2/workspaces/poll", &poll).await;
    let generation = source.generation + 1;
    let command = &launched["command"];
    assert_eq!(command["action"], start_action(generation));
    poll["report"] = json!({"command":command,"result":{
        "workspace":source.workspace,"phase":"running","generation":generation,"storageBytes":1_u64<<30
    }});
    let replied = post(fixture, "/daemon/v2/workspaces/poll", &poll).await;
    assert_eq!(replied["acknowledged"][0], command["request"]);
    let code = json!({"device":source.binding.destination, "workspaces":[
        {"workspace":source.workspace,"generation":generation,"ready":true}
    ],"reports":[]});
    assert!(post(fixture, "/daemon/v1/code/poll", &code).await["command"].is_null());
    let enabled = fixture.daemons.operate(
        "mutation($device:ID!, $input:ManagedCodeWorkspaceInput!){setManagedCodeWorkspaceEnabled(device:$device,input:$input)}",
        json!({"device":source.binding.destination,"input":{"workspace":source.workspace,"enabled":true}}),
    ).await;
    assert_eq!(
        enabled["data"]["setManagedCodeWorkspaceEnabled"], true,
        "{enabled}"
    );
    (command.clone(), code)
}

async fn code_workspaces(fixture: &Fixture) -> Value {
    let value = fixture
        .daemons
        .operate(
            "query($device:ID!){managedCodeWorkspaces(device:$device)}",
            json!({"device":fixture.receipt.source.binding.destination}),
        )
        .await;
    assert!(value["errors"].is_null(), "{value}");
    value["data"]["managedCodeWorkspaces"][0]["continuation"].clone()
}

async fn post(fixture: &Fixture, path: &str, body: &Value) -> Value {
    let mut request = call(Method::POST, path, &fixture.destination);
    *request.body_mut() = Body::from(body.to_string());
    let response = send(&fixture.app, request, PERMITTED).await;
    let status = response.status();
    let body = contents(response).await;
    assert_eq!(status, StatusCode::OK, "{path}: {body:?}");
    serde_json::from_slice(&body).unwrap()
}

fn start_action(generation: u64) -> Value {
    json!({"kind":"start","generation":generation,"memory_bytes":MEMORY,"cpu_millis":CPU,"tasks":TASKS})
}

pub(super) async fn seed_session(pool: &PgPool, device: Uuid, source: Source, now: DateTime<Utc>) {
    sqlx::query("UPDATE agent_threads.workspace_requests SET payload=$3 WHERE device_id=$1 AND workspace=$2")
        .bind(device).bind(source.workspace).bind(json!({"action":start_action(source.generation)}))
        .execute(pool).await.unwrap();
    sqlx::query(
        "INSERT INTO agent_threads.workspaces(device_id,workspace,generation,ready,enabled,seen_at)
        VALUES($1,$2,1,true,true,$3)",
    )
    .bind(device)
    .bind(source.workspace)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    let command = Uuid::now_v7();
    let payload = json!({"request":command,"workspace":source.workspace,"principal":source.principal,
        "generation":source.generation,"action":{"call":"start","program":"codex","mode":"ask",
            "folder":"/home/agent/workspace","prompt":"Update the homepage","worktree":false}});
    sqlx::query("INSERT INTO agent_threads.commands
        (request,device_id,workspace,generation,principal,payload,state,result,created_at,expires_at)
        VALUES($1,$2,$3,1,$4,$5,'completed',$6,$7,$8)")
        .bind(command).bind(device).bind(source.workspace).bind(source.principal).bind(payload)
        .bind(json!({"outcome":"started","thread":command,"folder":"/home/agent/workspace"}))
        .bind(now).bind(now+TimeDelta::minutes(5)).execute(pool).await.unwrap();
    seed_history(pool, device, command, now).await;
}

async fn seed_history(pool: &PgPool, device: Uuid, command: Uuid, now: DateTime<Utc>) {
    let thread = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO agent_threads.summaries
        (id,device_id,runner_id,program,state,title,started_at,activity_at,revision)
        VALUES($1,$2,$3,'codex','working','Homepage',$4,$4,1)",
    )
    .bind(thread)
    .bind(device)
    .bind(command)
    .bind(now)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO agent_threads.entries(thread_id,sequence,content) VALUES ($1,1,$2),($1,2,$3)",
    )
    .bind(thread)
    .bind(json!({"change":"prompt","text":"Add a footer","source":"user"}))
    .bind(json!({"change":"said","event":{"kind":"tool_result","text":"private tool output"}}))
    .execute(pool)
    .await
    .unwrap();
}
