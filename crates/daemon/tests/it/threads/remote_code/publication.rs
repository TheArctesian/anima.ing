use std::sync::atomic::{AtomicBool, Ordering};

use agent_threads::control::{Action, Outcome, RunState};
use chrono::Utc;
use daemon_core::wire::{
    Batch, PushAccepted, ThreadSummaries, ThreadSummariesReceipt, ThreadSummaryReceipt,
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
        // Production retries invalid acknowledgements on the normal telemetry schedule.
        tokio::time::timeout(Duration::from_secs(150), self.published.notified())
            .await
            .expect("the expected publication phase should be reached");
    }

    pub(super) async fn restart(&self, kit: &fixture::RemoteKit) {
        self.wait().await;
        kit.restart().await;
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

pub(super) fn routes(router: Router<Hub>) -> Router<Hub> {
    router.route("/daemon/v1/push", post(push)).route(
        "/daemon/v1/threads/entries",
        post(|| async { StatusCode::SERVICE_UNAVAILABLE }),
    )
}

async fn push(
    State(hub): State<Hub>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Batch>,
) -> Response {
    k8::observe_renewed(hub.3.as_deref(), &headers);
    assert!(body.daemon_version.is_some());
    let accepted = body
        .check(DEVICE, Utc::now())
        .expect("a valid production K8 batch");
    let Some(summary) = accepted.agent_threads else {
        return StatusCode::NO_CONTENT.into_response();
    };
    if summary.summaries.is_empty() {
        return Json(json!({})).into_response();
    }
    if summary.summaries[0].state != "idle" {
        return Json(json!({})).into_response();
    }
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
        fixture_with_publication(Replies::Held(gate.clone())).await;
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
