mod compose;

use std::{
    collections::HashSet,
    path::Path,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use anima_applications_wire::{Command, Decision, Message, Reply, SOCKET};
use axum::{Json, Router, extract::State as AxumState, response::IntoResponse as _, routing::post};
use chrono::{SecondsFormat, TimeDelta};
use serde_json::json;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, UnixListener, UnixStream},
    sync::{Notify, mpsc},
};
use uuid::Uuid;

use super::*;
use crate::{
    config::origin::Origin,
    hub::{HubClient, TIMEOUT},
    runtime::source_work::{Coordinator, claiming},
    token_store::{Credential, TokenStore},
};

#[tokio::test]
#[ignore = "requires a root-owned disposable Linux container with no applications socket"]
async fn a_daemon_restart_after_docker_runs_reports_the_same_attempt_without_starting_twice() {
    let temporary = tempfile::tempdir().unwrap();
    let home = Home::at(temporary.path().join("daemon"));
    let device = Uuid::now_v7();
    let origin = Origin::parse("https://example.invalid").unwrap();
    let socket = Socket::bind();
    let invocations = Arc::new(AtomicUsize::new(0));
    let executed = Arc::new(Notify::new());
    let commands = tokio::spawn(owner(socket, invocations.clone(), executed.clone()));
    let item = requested();
    let (target, mut events) = hub(
        &home,
        (device, &origin),
        item.clone(),
        ApplyFixture::Delivered,
    )
    .await;
    let mut source = Coordinator::open(&home, device, &origin).unwrap();
    let mut first = Hosting::open(&home, &target).unwrap();

    claiming::poll(&mut source, &target, Some(&mut first))
        .await
        .unwrap();
    let first_claim = next(&mut events).await;
    assert_eq!(first_claim["holds"], json!([]));
    assert_eq!(first_claim["types"], json!(["container_action"]));
    tokio::time::timeout(Duration::from_secs(5), executed.notified())
        .await
        .unwrap();
    drop(first);
    drop(source);

    let mut source = Coordinator::open(&home, device, &origin).unwrap();
    let mut reopened = reopen(&home, &target).await;
    claiming::poll(&mut source, &target, Some(&mut reopened))
        .await
        .unwrap();
    let (claim, final_report) = recovered(&mut events).await;
    assert_eq!(claim["holds"], json!([{"id":item["id"],"attempt":1}]));
    assert_eq!(final_report["id"], item["id"]);
    assert_eq!(final_report["attempt"], 1);
    assert_eq!(final_report["result"]["cause"], "orphan");
    assert_eq!(invocations.load(Ordering::SeqCst), 1);
    assert!(
        reopened.commands.is_empty(),
        "the repeated claim scheduled a local action"
    );
    commands.abort();
    assert!(commands.await.unwrap_err().is_cancelled());
}

async fn reopen(home: &Home, target: &HubTarget) -> Hosting {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(hosting) = Hosting::open(home, target) {
                return hosting;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}

async fn recovered(events: &mut mpsc::UnboundedReceiver<Value>) -> (Value, Value) {
    let mut claim = None;
    let mut terminal = None;
    loop {
        let event = next(events).await;
        if event["applications"] == "on" {
            claim = Some(event);
        } else if event["state"] == "failed" {
            terminal = Some(event);
        }
        if let (Some(claim), Some(terminal)) = (&claim, &terminal) {
            return (claim.clone(), terminal.clone());
        }
    }
}

async fn next(events: &mut mpsc::UnboundedReceiver<Value>) -> Value {
    tokio::time::timeout(Duration::from_secs(5), events.recv())
        .await
        .unwrap()
        .unwrap()
}

fn requested() -> Value {
    json!({
        "id":Uuid::now_v7(), "type":"container_action", "kind":"host", "attempt":1,
        "lease":{"until":until()}, "project":null, "secrets":[],
        "limits":{"seconds":120,"idleSeconds":1,"memoryBytes":1,"diskBytes":1,"orphanSeconds":1},
        "parameters":{"container":"a".repeat(64),"action":"restart"}
    })
}

fn until() -> String {
    (Utc::now() + TimeDelta::seconds(90)).to_rfc3339_opts(SecondsFormat::Secs, true)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ApplyFixture {
    Delivered,
    DropCompose,
    DropEnvironment,
    UnconfirmedVersion,
    StopInHealth,
}

#[derive(Clone)]
struct Hub {
    item: Value,
    fixture: ApplyFixture,
    consumed: Arc<Mutex<HashSet<String>>>,
    events: mpsc::UnboundedSender<Value>,
}

async fn hub(
    home: &Home,
    (device, origin): (Uuid, &Origin),
    item: Value,
    fixture: ApplyFixture,
) -> (HubTarget, mpsc::UnboundedReceiver<Value>) {
    let (sent, events) = mpsc::unbounded_channel();
    let router = Router::new()
        .route(
            "/work/v1/claim",
            post(
                |AxumState(hub): AxumState<Hub>, Json(body): Json<Value>| async move {
                    hub.events.send(body).unwrap();
                    Json(hub.item)
                },
            ),
        )
        .route(
            "/work/v1/report",
            post(
                |AxumState(hub): AxumState<Hub>, Json(body): Json<Value>| async move {
                    let stop = (hub.fixture == ApplyFixture::StopInHealth
                        && body["state"] == "finishing")
                        .then(|| json!({"reason":"operator"}));
                    hub.events.send(body).unwrap();
                    Json(json!({"lease":{"until":until()},"stop":stop,"logEnd":0}))
                },
            ),
        )
        .route(
            "/work/v1/secret",
            post(|state, body| async { secret(state, body) }),
        )
        .with_state(Hub {
            item,
            events: sent,
            fixture,
            consumed: Arc::default(),
        });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}/", listener.local_addr().unwrap())
        .parse()
        .unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let tokens = TokenStore::open(home);
    tokens
        .save_new(&Credential {
            token: DaemonToken::from_random([19; 32]),
            expires_at: Utc::now() + TimeDelta::hours(1),
            hub: origin.clone(),
        })
        .unwrap();
    (
        HubTarget {
            hub: Arc::new(HubClient::new(base, TIMEOUT).unwrap()),
            device,
            origin: origin.clone(),
            tokens,
        },
        events,
    )
}

fn secret(AxumState(hub): AxumState<Hub>, Json(body): Json<Value>) -> axum::response::Response {
    let purpose = body["purpose"].as_str().unwrap();
    let value = match purpose {
        "compose_file" => compose::COMPOSE,
        "environment_file" => compose::ENVIRONMENT,
        _ => panic!("an unknown file purpose"),
    };
    hub.events.send(body.clone()).unwrap();
    if !hub.consumed.lock().unwrap().insert(purpose.to_owned()) {
        return (
            axum::http::StatusCode::CONFLICT,
            Json(json!({"error":"SECRET_REFUSED"})),
        )
            .into_response();
    }
    if matches!(
        (hub.fixture, purpose),
        (ApplyFixture::DropCompose, "compose_file")
            | (ApplyFixture::DropEnvironment, "environment_file")
    ) {
        let dropped = std::io::Error::from(std::io::ErrorKind::ConnectionReset);
        let stream = futures_util::stream::once(async { Err::<bytes::Bytes, _>(dropped) });
        return axum::body::Body::from_stream(stream).into_response();
    }
    Json(json!({"value":value,"expiresAt":until()})).into_response()
}

struct Socket {
    listener: UnixListener,
}

impl Socket {
    fn bind() -> Self {
        let path = Path::new(SOCKET);
        std::fs::create_dir(path.parent().unwrap()).unwrap();
        Self {
            listener: UnixListener::bind(path).unwrap(),
        }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        let path = Path::new(SOCKET);
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(path.parent().unwrap());
    }
}

async fn owner(socket: Socket, invocations: Arc<AtomicUsize>, executed: Arc<Notify>) {
    while let Ok((stream, _)) = socket.listener.accept().await {
        tokio::spawn(on_socket(stream, invocations.clone(), executed.clone()));
    }
}

async fn on_socket(mut stream: UnixStream, invocations: Arc<AtomicUsize>, executed: Arc<Notify>) {
    let message: Message = read(&mut stream).await;
    match message.command {
        Command::Probe => return write_reply(&mut stream, &Reply::Paired).await,
        Command::ProbeApply => {
            return write_reply(
                &mut stream,
                &Reply::Refused {
                    cause: "member_unknown".to_owned(),
                },
            )
            .await;
        }
        Command::Item { .. } => (),
    }
    write_reply(&mut stream, &Reply::Ready).await;
    assert!(matches!(
        read::<Decision>(&mut stream).await,
        Decision::Start
    ));
    invocations.fetch_add(1, Ordering::SeqCst);
    let status = tokio::process::Command::new("/bin/true")
        .status()
        .await
        .unwrap();
    assert!(status.success());
    executed.notify_one();
    // The simulated Docker command ran. Leave the reply unread until the daemon shuts down.
    assert_eq!(stream.read(&mut [0; 1]).await.unwrap(), 0);
}

async fn read<T: serde::de::DeserializeOwned>(stream: &mut UnixStream) -> T {
    let length = stream.read_u32().await.unwrap() as usize;
    let mut body = vec![0; length];
    stream.read_exact(&mut body).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn write_reply(stream: &mut UnixStream, reply: &Reply) {
    let body = serde_json::to_vec(reply).unwrap();
    stream
        .write_u32(u32::try_from(body.len()).unwrap())
        .await
        .unwrap();
    stream.write_all(&body).await.unwrap();
}
