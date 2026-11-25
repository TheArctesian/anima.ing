use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicBool, Ordering},
};

use agent_threads::control::{Action, Outcome, RunState};
use anima_daemon::{
    config::agent_usage::UsageSettings,
    hub::{HubClient, TIMEOUT},
    push::{Event, Pusher},
};
use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::{
    points::{Kind, Point, SeriesKey, Window},
    wire::{
        Batch, PushAccepted, ThreadSummaries, ThreadSummariesReceipt, ThreadSummary,
        ThreadSummaryReceipt,
    },
};
use tokio::sync::Notify;

use super::*;

#[derive(Clone)]
pub(super) enum Replies {
    Normal,
    Retry(Arc<Retry>),
    Held(Arc<Held>),
}

#[derive(Default)]
pub(super) struct Retry {
    pub(super) ack: Held,
    pub(super) running: Held,
    first_running: AtomicBool,
}

#[derive(Default)]
pub(super) struct Held {
    published: Notify,
    release: Notify,
}

impl Held {
    async fn pause(&self) {
        self.published.notify_one();
        self.release.notified().await;
    }

    async fn wait(&self) {
        tokio::time::timeout(Duration::from_secs(60), self.published.notified())
            .await
            .expect("the expected publication phase should be reached");
    }

    pub(super) async fn restart(&self, kit: &mut Kit) {
        self.wait().await;
        kit.end_runner();
        kit.start();
        self.release.notify_one();
    }
}

pub(super) async fn before_running_reply(hub: &Hub, body: &Value) {
    if let Replies::Retry(retry) = &hub.2
        && body["state"] == "running"
        && !retry.first_running.swap(true, Ordering::SeqCst)
    {
        retry.running.pause().await;
    }
}

pub(super) async fn sample(home: Home, hub: Hub) {
    let Some(runner) = ready(&home, &hub).await else {
        return;
    };
    let mut pusher = publisher(&home);
    let policy = UsageSettings::read(&home).unwrap();
    pusher.accept(Event::ThreadSummaries(Some((
        snapshot(runner),
        policy,
        BTreeSet::from([runner]),
    ))));
    let mut wait = Duration::ZERO;
    loop {
        tokio::time::sleep(wait.min(Duration::from_secs(1))).await;
        let now = Utc::now();
        pusher.accept(Event::Point(point(now)));
        wait = pusher.push_due(now).await;
    }
}

fn point(now: DateTime<Utc>) -> Point {
    let at = DateTime::from_timestamp(now.timestamp().div_euclid(10) * 10, 0).unwrap();
    let cpu = SeriesKey::new(Kind::CpuPercent, None, None).unwrap();
    let mut window = Window::new();
    assert!(window.add(at, [(cpu, 1.0)]).is_none());
    window.add(at + TimeDelta::seconds(1), []).unwrap()
}

fn publisher(home: &Home) -> Pusher {
    let store = TokenStore::open(home);
    let mut credential = store.load().unwrap().unwrap();
    credential.expires_at = Utc::now() + TimeDelta::days(365);
    let link = HubClient::over_link(&home.link_socket(), TIMEOUT).unwrap();
    Pusher::new(link, DEVICE, credential, store).with_marks(home.clone())
}

async fn ready(home: &Home, hub: &Hub) -> Option<Uuid> {
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let runner = hub.0.lock().unwrap().runner;
            if let Some(runner) = runner
                && publication_requested(home)
            {
                return runner;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .ok()
}

fn publication_requested(home: &Home) -> bool {
    let journal = std::fs::read(home.dir().join("remote-code.json")).ok();
    let Some(journal) = journal.and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
    else {
        return false;
    };
    journal["launches"][LAUNCH.to_string()]["publication"].is_object()
}

fn snapshot(runner_id: Uuid) -> ThreadSummaries {
    let now = Utc::now();
    let revision = now.timestamp_micros() + 1_000_000;
    let time = DateTime::from_timestamp(now.timestamp(), 0).unwrap();
    ThreadSummaries {
        count: 1,
        count_revision: revision,
        summaries: vec![ThreadSummary {
            runner_id,
            program: "claude_code".to_owned(),
            state: "idle".to_owned(),
            title: String::new(),
            started_at: time,
            activity_at: time,
            revision,
            repository: None,
        }],
    }
}

pub(super) async fn push(State(hub): State<Hub>, Json(body): Json<Batch>) -> Response {
    assert!(body.daemon_version.is_some());
    let accepted = body
        .check(DEVICE, Utc::now())
        .expect("a valid production K8 batch");
    let Some(summary) = accepted.agent_threads else {
        return StatusCode::NO_CONTENT.into_response();
    };
    let (attempt, mut receipt) = observe(&hub, &summary);
    match &hub.2 {
        Replies::Retry(_) if attempt == 1 => return StatusCode::BAD_GATEWAY.into_response(),
        Replies::Retry(_) if attempt == 2 => return Json(json!({})).into_response(),
        Replies::Retry(_) if attempt == 3 => receipt.summaries[0].revision -= 1,
        Replies::Retry(_) if attempt == 4 => receipt.summaries[0].runner_id = Uuid::now_v7(),
        Replies::Retry(retry) if attempt == 5 => retry.ack.pause().await,
        Replies::Held(gate) => gate.pause().await,
        _ => {}
    }
    Json(PushAccepted {
        agent_threads: Some(receipt),
        keys: None,
        update: None,
        account_salt: None,
    })
    .into_response()
}

fn observe(hub: &Hub, batch: &ThreadSummaries) -> (u32, ThreadSummariesReceipt) {
    let mut state = hub.0.lock().unwrap();
    assert_eq!(batch.count, 1);
    assert_eq!(batch.summaries.len(), 1);
    let row = &batch.summaries[0];
    assert_eq!(Some(row.runner_id), state.runner);
    state.summary_attempts += 1;
    if state.summary_attempts <= 4 && matches!(hub.2, Replies::Retry(_)) {
        assert!(
            !state.done,
            "work finished before its explicit K8 acknowledgement"
        );
    }
    state.summary = true;
    (
        state.summary_attempts,
        ThreadSummariesReceipt {
            count_revision: batch.count_revision,
            summaries: vec![ThreadSummaryReceipt {
                runner_id: row.runner_id,
                id: THREAD,
                revision: row.revision,
            }],
        },
    )
}

#[tokio::test]
async fn th32_withdrawing_sharing_before_k8_ack_closes_the_binding() {
    let gate = Arc::new(Held::default());
    let (kit, home, folder, hub, serving) =
        fixture_with_publication(None, Replies::Held(gate.clone()));
    gate.wait().await;
    private_file::write(
        home.dir(),
        "thread-sync.json",
        br#"{"shareTranscripts":false}"#,
    )
    .unwrap();
    gate.release.notify_one();
    wait_for(&hub, |launch| launch.withdrawn).await;
    assert!(folder.join("started").is_file());
    assert!(!folder.join("effects").exists());
    let mut client = kit.client().await;
    let id = client.call(Action::List).await;
    let Outcome::Threads { threads, .. } = client.done(id).await else {
        panic!("the local runner should still list the thread");
    };
    assert!(threads.iter().any(|row| row.state == RunState::Idle));
    let state = hub.0.lock().unwrap();
    assert!(!state.done);
    assert_eq!(state.delivered, 0);
    serving.abort();
}
