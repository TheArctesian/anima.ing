//! Exercise the daemon claim, one-use file delivery and the authenticated owner socket.
use std::sync::atomic::AtomicBool;

use anima_applications_wire::Health;

use super::*;

pub(super) const COMPOSE: &str = "services:\n  web:\n    image: example/web:1\n";
pub(super) const ENVIRONMENT: &str = "SYNTHETIC=yes\n";

#[tokio::test]
#[ignore = "requires a root-owned disposable Linux container with no applications socket"]
async fn dropping_a_consumed_file_fails_once_without_an_owner_apply() {
    let scenarios = [
        (ApplyFixture::DropCompose, &["compose_file"][..]),
        (
            ApplyFixture::DropEnvironment,
            &["compose_file", "environment_file"][..],
        ),
    ];
    for (fixture, expected) in scenarios {
        let (events, applications, _) = exchange(fixture).await;
        assert_eq!(requested_files(&events), expected);
        assert_eq!(
            applications, 0,
            "the daemon sent an applications-service apply"
        );
        let end = events.last().unwrap();
        assert_eq!(end["state"], "failed");
        assert_eq!(end["result"]["outcome"], "failed");
        // This work-route cause tells Hosting to request a fresh submission.
        assert_eq!(end["result"]["cause"], "secret_failed");
        for fetch in events.iter().filter(|event| event["purpose"].is_string()) {
            assert_eq!(end["id"], fetch["id"]);
            assert_eq!(end["attempt"], fetch["attempt"]);
        }
        assert!(
            events.iter().all(|event| event["state"] != "running"),
            "the daemon reported a start with an incomplete set of files"
        );
    }
}

#[tokio::test]
#[ignore = "requires a root-owned disposable Linux container with no applications socket"]
async fn a_complete_apply_reaches_the_owner_and_reports_the_health_phase() {
    let (events, applications, _) = exchange(ApplyFixture::Delivered).await;
    assert_eq!(
        requested_files(&events),
        ["compose_file", "environment_file"]
    );
    assert_eq!(applications, 1);
    let claim = events.first().unwrap();
    assert_eq!(
        claim["types"],
        json!(["container_action", "application_apply"])
    );
    let states: Vec<_> = events
        .iter()
        .filter_map(|event| event["state"].as_str())
        .collect();
    assert!(states.starts_with(&["preparing", "preparing", "running", "finishing"]));
    let end = events.last().unwrap();
    assert_eq!(end["state"], "done");
    assert_eq!(end["result"]["health"], "healthy");
}

#[tokio::test]
#[ignore = "requires a root-owned disposable Linux container with no applications socket"]
async fn an_unconfirmed_owner_result_reaches_the_hub_as_orphan() {
    let (events, applications, _) = exchange(ApplyFixture::UnconfirmedVersion).await;
    assert_eq!(applications, 1);
    let end = events.last().unwrap();
    assert_eq!(end["state"], "failed");
    assert_eq!(end["result"]["cause"], "orphan");
}

#[tokio::test]
#[ignore = "requires a root-owned disposable Linux container with no applications socket"]
async fn a_stop_during_finishing_closes_the_owner_write_side_and_reports_stopped() {
    let (events, applications, owner_closed) = exchange(ApplyFixture::StopInHealth).await;
    assert_eq!(applications, 1);
    assert!(owner_closed);
    assert!(events.iter().any(|event| event["state"] == "finishing"));
    assert_eq!(events.last().unwrap()["state"], "stopped");
}

fn requested_files(events: &[Value]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|event| event["purpose"].as_str())
        .collect()
}

async fn exchange(fixture: ApplyFixture) -> (Vec<Value>, usize, bool) {
    let temp = tempfile::tempdir().unwrap();
    let home = Home::at(temp.path().join("daemon"));
    let origin = Origin::parse("https://example.invalid").unwrap();
    let device = Uuid::now_v7();
    let socket = Socket::bind();
    let invocations = Arc::new(AtomicUsize::new(0));
    let closed = Arc::new(AtomicBool::new(false));
    let server = tokio::spawn(serve(
        socket,
        (invocations.clone(), closed.clone()),
        fixture,
    ));
    let item = requested_apply();
    let (target, mut events) = hub(&home, (device, &origin), item, fixture).await;
    let mut source = Coordinator::open(&home, device, &origin).unwrap();
    let mut hosting = Hosting::open(&home, &target).unwrap();

    let events = tokio::time::timeout(Duration::from_secs(5), async {
        let mut recorded = Vec::new();
        loop {
            claiming::poll(&mut source, &target, Some(&mut hosting))
                .await
                .unwrap();
            while let Ok(event) = events.try_recv() {
                let terminal =
                    matches!(event["state"].as_str(), Some("done" | "failed" | "stopped"));
                recorded.push(event);
                if terminal {
                    return recorded;
                }
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
    (
        events,
        invocations.load(Ordering::SeqCst),
        closed.load(Ordering::SeqCst),
    )
}

fn requested_apply() -> Value {
    use crate::model::fetch::hex;
    use sha2::{Digest as _, Sha256};
    json!({
        "id":Uuid::now_v7(), "type":"application_apply", "kind":"host", "attempt":1,
        "lease":{"until":until()}, "project":null, "secrets":["compose_file","environment_file"],
        "limits":{"seconds":120,"idleSeconds":0,"memoryBytes":0,"diskBytes":0,"orphanSeconds":0},
        "parameters":{"application":Uuid::now_v7(), "fileSha256":hex(&Sha256::digest(COMPOSE)),
            "environmentSha256":hex(&Sha256::digest(ENVIRONMENT)), "pull":true, "keep":0}
    })
}

async fn serve(
    socket: Socket,
    (invocations, closed): (Arc<AtomicUsize>, Arc<AtomicBool>),
    fixture: ApplyFixture,
) {
    while let Ok((stream, _)) = socket.listener.accept().await {
        tokio::spawn(one(stream, (invocations.clone(), closed.clone()), fixture));
    }
}

async fn one(
    mut stream: UnixStream,
    (invocations, closed): (Arc<AtomicUsize>, Arc<AtomicBool>),
    fixture: ApplyFixture,
) {
    let message: Message = read(&mut stream).await;
    match message.command {
        Command::Probe => write_reply(&mut stream, &Reply::Paired).await,
        Command::ProbeApply => write_reply(&mut stream, &Reply::ApplyAvailable).await,
        Command::Item { json } => {
            let item: Value = serde_json::from_str(&json).unwrap();
            assert_eq!(item["type"], "application_apply");
            invocations.fetch_add(1, Ordering::SeqCst);
            apply(&mut stream, &closed, fixture).await;
        }
    }
}

async fn apply(stream: &mut UnixStream, closed: &AtomicBool, fixture: ApplyFixture) {
    receive_and_start(stream).await;
    if fixture == ApplyFixture::UnconfirmedVersion {
        write_reply(
            stream,
            &Reply::Failed {
                cause: "orphan".to_owned(),
            },
        )
        .await;
        return;
    }
    write_reply(stream, &Reply::CheckingHealth).await;
    if fixture == ApplyFixture::StopInHealth {
        assert_eq!(stream.read(&mut [0; 1]).await.unwrap(), 0);
        closed.store(true, Ordering::SeqCst);
        return;
    }

    write_reply(
        stream,
        &Reply::Applied {
            health: Health::Healthy,
        },
    )
    .await;
}

async fn receive_and_start(stream: &mut UnixStream) {
    write_reply(stream, &Reply::NeedsFiles).await;
    for expected in [COMPOSE, ENVIRONMENT] {
        let length = stream.read_u32().await.unwrap() as usize;
        let mut bytes = vec![0_u8; length];
        stream.read_exact(&mut bytes).await.unwrap();
        assert_eq!(bytes, expected.as_bytes());
    }
    write_reply(stream, &Reply::Ready).await;
    assert!(matches!(read::<Decision>(stream).await, Decision::Start));
}
