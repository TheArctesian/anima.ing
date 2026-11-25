//! Synthetic K10 and K8 across the real runner and holder sockets.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use anima_daemon::{
    config::{Config, home::Home, origin::Origin, private_file},
    token_store::{Credential, TokenStore},
};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use daemon_core::wire::{DaemonToken, Lease};
use serde_json::{Value, json};
use uuid::Uuid;

use super::runner::Kit;

mod commands;
mod publication;
mod races;

const DEVICE: Uuid = Uuid::from_u128(1);
const LAUNCH: Uuid = Uuid::from_u128(2);
const WORK: Uuid = Uuid::from_u128(3);
const PROJECT: Uuid = Uuid::from_u128(4);
const ACCOUNT: Uuid = Uuid::from_u128(5);
const GRANT: Uuid = Uuid::from_u128(6);
const THREAD: Uuid = Uuid::from_u128(7);
const COMMAND: &str = "../β:prompt";

const CLAUDE: &str = r#"#!/usr/bin/perl
$| = 1;
open(my $started, ">", "started") or die "startup marker"; close($started);
while (my $line = <STDIN>) {
    if ($line =~ /"type":"user"/) {
        open(my $file, ">>", "effects"); print $file "prompt\n"; close($file);
        print qq({"type":"system","subtype":"session_state_changed","state":"idle"}\n);
    }
}
"#;

#[derive(Clone)]
struct Hub(
    Arc<Mutex<Launch>>,
    Option<Arc<races::DispatchGate>>,
    publication::Replies,
);

#[derive(Debug)]
struct Launch {
    claims: Vec<(Value, Value)>,
    folder: PathBuf,
    claimed: bool,
    runner: Option<Uuid>,
    summary: bool,
    summary_attempts: u32,
    done: bool,
    delivered: u32,
    receipts: u32,
    withdrawn: bool,
    refusal: Option<String>,
}

#[tokio::test]
async fn th32_remote_code_claim_spawn_publish_receipt_and_withdrawal() {
    let retry = Arc::new(publication::Retry::default());
    let (mut kit, home, folder, hub, serving) =
        fixture_with_publication(None, publication::Replies::Retry(retry.clone()));
    retry.ack.restart(&mut kit).await;
    retry.running.restart(&mut kit).await;
    wait_for(&hub, |launch| launch.receipts == 2).await;
    assert_eq!(
        std::fs::read_to_string(folder.join("effects")).unwrap(),
        "prompt\n"
    );
    private_file::write(
        home.dir(),
        "thread-sync.json",
        br#"{"shareTranscripts":false}"#,
    )
    .unwrap();
    wait_for(&hub, |launch| launch.withdrawn).await;
    let observed = hub.0.lock().unwrap();
    assert_eq!(observed.delivered, 2);
    assert!(observed.summary_attempts >= 5);
    serving.abort();
}

fn fixture(grant: Option<Arc<races::DispatchGate>>) -> Fixture {
    fixture_with_publication(grant, publication::Replies::Normal)
}

fn fixture_with_publication(
    grant: Option<Arc<races::DispatchGate>>,
    replies: publication::Replies,
) -> Fixture {
    let mut kit = Kit::unstarted(CLAUDE);
    let home = Home::at(kit.path("daemon"));
    let folder = kit.path("work").canonicalize().unwrap();
    let listener = synthetic_enrollment(&home, &folder);
    let hub = Hub(
        Arc::new(Mutex::new(Launch {
            folder: folder.clone(),
            claims: Vec::new(),
            claimed: false,
            runner: None,
            summary: false,
            summary_attempts: 0,
            done: false,
            delivered: 0,
            receipts: 0,
            withdrawn: false,
            refusal: None,
        })),
        grant,
        replies,
    );
    let router = Router::new()
        .route("/work/v1/claim", post(claim))
        .route("/work/v1/report", post(report_work))
        .route("/work/v1/thread-dispatch", post(dispatch))
        .route("/work/v1/thread-launch-evidence", post(evidence))
        .route("/work/v1/commands", post(commands::poll))
        .route("/work/v1/command-result", post(commands::receipt))
        .route(
            "/work/v1/thread-control",
            post(|| async { StatusCode::NO_CONTENT }),
        )
        .route("/daemon/v1/push", post(publication::push))
        .with_state(hub.clone());
    let pushing = publication::sample(home.clone(), hub.clone());
    let serving = tokio::spawn(async move {
        tokio::select! {
            result = axum::serve(listener, router) => result.unwrap(),
            () = pushing => {},
        }
    });
    kit.start();
    (kit, home, folder, hub, serving)
}

type Fixture = (Kit, Home, PathBuf, Hub, tokio::task::JoinHandle<()>);

fn synthetic_enrollment(home: &Home, folder: &std::path::Path) -> tokio::net::UnixListener {
    use std::os::unix::fs::PermissionsExt as _;
    home.ensure().unwrap();
    let socket = home.link_socket();
    let listener = tokio::net::UnixListener::bind(&socket).unwrap();
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).unwrap();
    let origin = Origin::parse_kept(&format!("unix:{}", socket.display())).unwrap();
    home.write_config(&Config {
        hub: origin.clone(),
        device: DEVICE,
        device_name: "fixture".to_owned(),
    })
    .unwrap();
    let now = chrono::Utc::now();
    TokenStore::open(home)
        .save(&Credential::received(
            DaemonToken::from_random([3; 32]),
            now + chrono::TimeDelta::hours(1),
            origin,
            now,
        ))
        .unwrap();
    let settings = json!({
        "threads": true, "ceiling": "ask", "allowedFolders": [folder],
        "targets": [{ "project": PROJECT, "folder": folder, "program": "claude_code" }],
    });
    private_file::write(
        home.dir(),
        "remote-code-settings.json",
        &serde_json::to_vec(&settings).unwrap(),
    )
    .unwrap();
    private_file::write(
        home.dir(),
        "thread-sync.json",
        br#"{"shareTranscripts":true}"#,
    )
    .unwrap();
    listener
}

async fn wait_for(hub: &Hub, reached: impl Fn(&Launch) -> bool) {
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            if reached(&hub.0.lock().unwrap()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("remote fixture stalled: {:?}", hub.0.lock().unwrap()));
}

async fn claim(State(hub): State<Hub>, Json(body): Json<Value>) -> Response {
    let mut state = hub.0.lock().unwrap();
    state
        .claims
        .push((body["free"]["slots"].clone(), body["threadTargets"].clone()));
    if state.claimed || body["free"]["slots"] != 1 {
        return StatusCode::NO_CONTENT.into_response();
    }
    assert_eq!(body["switch"], "off");
    assert_eq!(body["threads"], "on");
    assert_eq!(body["threadTargets"][0]["project"], PROJECT.to_string());
    state.claimed = true;
    Json(json!({
        "id": WORK, "type": "agent_thread", "kind": "person", "attempt": 1,
        "lease": lease(),
        "limits": { "seconds": 120, "idleSeconds": 0, "memoryBytes": 0, "diskBytes": 0, "orphanSeconds": 0 },
        "project": PROJECT,
        "parameters": { "launch": LAUNCH, "account": ACCOUNT, "program": "claude_code",
            "folder": state.folder, "access": "ask" },
    })).into_response()
}

fn lease() -> Lease {
    Lease {
        until: chrono::Utc::now() + chrono::TimeDelta::seconds(90),
    }
}

async fn dispatch(
    State(hub): State<Hub>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    record_dispatch(&hub, &body);
    if let Some(gate) = &hub.1
        && let Some(final_reply) = gate.hold(&headers).await
    {
        return final_reply;
    }
    Json(json!({"authorization": GRANT, "validUntil": chrono::Utc::now(), "remainingMs": 10_000}))
        .into_response()
}

fn record_dispatch(hub: &Hub, body: &Value) {
    let mut state = hub.0.lock().unwrap();
    assert!(state.claimed);
    assert_eq!(body["device"], DEVICE.to_string());
    assert_eq!(body["launch"], LAUNCH.to_string());
    assert_eq!(body["work"], WORK.to_string());
    assert_eq!(body["attempt"], 1);
    assert!(body.get("account").is_none());
    let runner: Uuid = serde_json::from_value(body["runner"].clone()).unwrap();
    assert!(state.runner.is_none_or(|expected| expected == runner));
    state.runner = Some(runner);
}

async fn report_work(State(hub): State<Hub>, Json(body): Json<Value>) -> Json<Value> {
    publication::before_running_reply(&hub, &body).await;
    let mut state = hub.0.lock().unwrap();
    if body["state"] == "done" {
        assert!(state.summary);
        assert_eq!(body["result"]["thread"], state.runner.unwrap().to_string());
        state.done = true;
    }
    Json(json!({"lease":lease(), "logEnd":0, "stop":null}))
}

async fn evidence(State(hub): State<Hub>, Json(body): Json<Value>) -> Json<Value> {
    let mut state = hub.0.lock().unwrap();
    assert_eq!(body["launch"], LAUNCH.to_string());
    if body["outcome"] == "not_created" {
        assert!(body.get("thread").is_none());
        let cause = body["cause"].as_str().unwrap();
        state.refusal = Some(cause.to_owned());
        return Json(
            json!({"launch":LAUNCH, "state":cause, "cause":cause, "thread":null,
            "binding":"unbound", "firstCommand":Uuid::from_u128(8)}),
        );
    }
    assert_eq!(body["thread"], state.runner.unwrap().to_string());
    state.withdrawn |= body["publication"] == "withdrawn";
    Json(
        json!({"launch":LAUNCH, "state":"started", "cause":null, "thread":THREAD,
        "binding":"bound", "firstCommand":Uuid::from_u128(8)}),
    )
}

#[tokio::test]
async fn th32_same_user_client_cannot_open_the_remote_holder_exchange() {
    let holder = super::Holder::start(0);
    let mut talk = holder.link().await;
    use anima_daemon::threads::wire::{Action, Event};
    talk.link
        .send(Action::RemoteCode {
            frame: r#"{"op":"open"}"#.to_owned(),
        })
        .await
        .unwrap();
    let watch = async {
        while let Some(event) = talk.link.events.recv().await {
            assert!(
                !matches!(event, Event::RemoteCode { .. }),
                "same-user test process received a remote grant challenge"
            );
        }
    };
    assert!(
        tokio::time::timeout(Duration::from_millis(350), watch)
            .await
            .is_err()
    );
}
