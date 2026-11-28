use std::sync::atomic::{AtomicBool, Ordering};

use axum::http::{HeaderMap, header::AUTHORIZATION};
use tokio::sync::Notify;

use super::*;

#[derive(Default)]
pub(super) struct DispatchGate {
    answered: AtomicBool,
    arrived: Notify,
    release: Notify,
}

impl DispatchGate {
    pub(super) async fn hold(&self, headers: &HeaderMap) -> Option<Response> {
        let retry = self.answered.swap(true, Ordering::SeqCst);
        let token = DaemonToken::from_random([if retry { 4 } else { 3 }; 32]);
        assert!(
            headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                == Some(format!("Bearer {}", token.expose()).as_str()),
            "the dispatch request used the wrong fixture credential"
        );
        if retry {
            return Some(
                (
                    StatusCode::CONFLICT,
                    Json(json!({"error":"NOT_HOLDER", "authorization":GRANT})),
                )
                    .into_response(),
            );
        }
        self.arrived.notify_one();
        self.release.notified().await;
        None
    }

    async fn wait(&self) {
        tokio::time::timeout(Duration::from_secs(60), self.arrived.notified())
            .await
            .expect("the holder should obtain an original dispatch request");
    }
}

#[tokio::test]
async fn th32_removing_a_target_during_authorization_prevents_spawn() {
    let gate = Arc::new(DispatchGate::default());
    let (_kit, home, folder, hub, serving) = fixture(Some(gate.clone())).await;
    gate.wait().await;
    let settings = json!({
        "threads": true, "ceiling": "ask", "allowedFolders": [folder], "targets": [],
    });
    private_file::write(
        home.dir(),
        "remote-code-settings.json",
        &serde_json::to_vec(&settings).unwrap(),
    )
    .unwrap();
    gate.release.notify_one();
    no_child(&hub, &folder, "refused").await;
    serving.abort();
}

#[tokio::test]
async fn th32_renewal_rejects_an_authorization_started_with_the_old_credential() {
    let gate = Arc::new(DispatchGate::default());
    let renewal = Arc::new(k8::Renewal::default());
    let (_kit, _home, folder, hub, serving) = fixture::fixture_with_options(
        Some(gate.clone()),
        publication::Replies::Normal,
        None,
        Some(renewal.clone()),
    )
    .await;
    gate.wait().await;
    renewal.allow_and_observe().await;
    gate.release.notify_one();
    no_child(&hub, &folder, "expired").await;
    serving.abort();
}

async fn no_child(hub: &Hub, folder: &std::path::Path, cause: &str) {
    wait_for(hub, |launch| launch.summary || launch.refusal.is_some()).await;
    assert!(!folder.join("started").exists(), "the provider was spawned");
    assert_eq!(hub.0.lock().unwrap().refusal.as_deref(), Some(cause));
}
