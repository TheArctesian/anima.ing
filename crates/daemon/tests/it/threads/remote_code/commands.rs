use std::sync::atomic::{AtomicBool, Ordering};

use anima_daemon::threads::link::HolderLink;
use tokio::sync::Notify;

use super::*;

pub(super) async fn poll(State(hub): State<Hub>) -> Response {
    let gate = hub.0.lock().unwrap().poll_gate.clone();
    if let Some(gate) = gate {
        gate.deliver().await;
        return StatusCode::NO_CONTENT.into_response();
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    let mut state = hub.0.lock().unwrap();
    if !state.done || state.delivered == 2 || state.delivered != state.receipts {
        return StatusCode::NO_CONTENT.into_response();
    }
    state.delivered += 1;
    Json(json!({"commands":[{
        "id":COMMAND, "account":ACCOUNT, "device":DEVICE, "launch":LAUNCH, "thread":state.runner,
        "payload":{"kind":"prompt", "prompt":"fixture"},
    }]}))
    .into_response()
}

pub(super) async fn receipt(State(hub): State<Hub>, Json(body): Json<Value>) -> StatusCode {
    assert_eq!(body["id"], COMMAND);
    assert_eq!(body["outcome"], "recorded");
    hub.0.lock().unwrap().receipts += 1;
    StatusCode::OK
}

#[derive(Debug, Default)]
pub(super) struct PollGate {
    started: AtomicBool,
    first: Notify,
    release: Notify,
    resumed: Notify,
}

impl PollGate {
    async fn deliver(&self) {
        if self.started.swap(true, Ordering::SeqCst) {
            self.resumed.notify_one();
        } else {
            self.first.notify_one();
            self.release.notified().await;
        }
    }
}

#[tokio::test]
async fn th32_holder_reconnect_keeps_the_existing_command_poll_slot() {
    let (kit, _home, _folder, hub, serving) = fixture(None);
    let gate = Arc::new(PollGate::default());
    {
        let mut state = hub.0.lock().unwrap();
        state.poll_gate = Some(gate.clone());
        // The launch interval remains enabled but the hub has no work for this device.
        state.claimed = true;
    }
    tokio::time::timeout(Duration::from_secs(60), gate.first.notified())
        .await
        .expect("the first eligible poll should enter the hub");
    wait_for(&hub, |state| ready_claims(state) > 0).await;
    let before = ready_claims(&hub.0.lock().unwrap());
    let replacement = HolderLink::connect(&kit.threads()).await.unwrap();
    drop(replacement);
    tokio::time::timeout(Duration::from_secs(24), async {
        loop {
            if ready_claims(&hub.0.lock().unwrap()) > before {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("the daemon should authenticate its replacement holder connection");
    assert!(
        tokio::time::timeout(Duration::from_millis(500), gate.resumed.notified())
            .await
            .is_err(),
        "the new holder connection opened a second concurrent HTTP command poll"
    );
    gate.release.notify_one();
    tokio::time::timeout(Duration::from_secs(5), gate.resumed.notified())
        .await
        .expect("the slot should reopen once the original poll returns");
    serving.abort();
}

fn ready_claims(state: &Launch) -> usize {
    state
        .claims
        .iter()
        .filter(|(slots, _)| slots.as_u64() == Some(1))
        .count()
}
