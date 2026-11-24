//! The calls of the daemon to a `llama_server` router on loopback (RTC-005, K11 Section 11.1): its list of copies,
//! a load, an unload and the counters of a copy. The router answers a load or an unload at once and then works,
//! so the daemon reads the list until the copy is in the state that it wants.

use std::time::Duration;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::router::{Status, Use, statuses, usage};
use reqwest::Client;
use serde_json::{Value, json};
use uuid::Uuid;
use zeroize::Zeroizing;

/// The longest wait for a load or an unload: the limit of `start_failed`.
pub const WAIT_SECONDS: u64 = 120;
const POLL: Duration = Duration::from_millis(500);

/// A server of the node in router mode, as the daemon reaches it.
#[derive(Clone)]
pub struct Router {
    pub port: u16,
    pub key: Zeroizing<String>,
    client: Client,
}

/// How a wait ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waited {
    Reached,
    /// The router ended the load with an error of the runtime, or the copy left the list.
    Failed,
    TimedOut,
    /// The caller stopped the wait (a return gives way to a load).
    Stopped,
}

impl Router {
    #[must_use]
    pub fn new(port: u16, key: Zeroizing<String>) -> Option<Self> {
        let client = Client::builder()
            .no_proxy()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(10))
            .build()
            .ok()?;
        Some(Self { port, key, client })
    }

    fn address(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    async fn get(&self, path: &str) -> Option<String> {
        let answer = self
            .client
            .get(self.address(path))
            .bearer_auth(self.key.as_str())
            .send()
            .await
            .ok()?;
        if !answer.status().is_success() {
            return None;
        }
        answer.text().await.ok()
    }

    /// The copies of the preset and their states; `reload` reads the preset again first.
    pub async fn statuses(&self, reload: bool) -> Option<Vec<(Uuid, Status)>> {
        let path = if reload {
            "/models?reload=1"
        } else {
            "/models"
        };
        let text = self.get(path).await?;
        let answer: Value = serde_json::from_str(&text).ok()?;
        Some(statuses(&answer))
    }

    /// The counters of `copy`; `None` when it is not loaded or the answer does not read.
    pub async fn usage(&self, copy: Uuid) -> Option<Use> {
        usage(&self.get(&format!("/metrics?model={copy}")).await?)
    }

    /// `POST /models/load` or `/models/unload`. A 400 says that the copy is already in that state.
    ///
    /// # Errors
    /// `start_failed` when the router does not answer, or answers with an other error.
    pub async fn ask(&self, path: &str, copy: Uuid) -> Result<(), Cause> {
        let answer = self
            .client
            .post(self.address(path))
            .bearer_auth(self.key.as_str())
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(json!({ "model": copy }).to_string())
            .send()
            .await
            .map_err(|_| Cause::StartFailed)?;
        match answer.status().as_u16() {
            200 | 400 => Ok(()),
            _ => Err(Cause::StartFailed),
        }
    }

    /// Reads the list each half second until `copy` is in the state that `reached` wants, for at most
    /// [`WAIT_SECONDS`] in all; `stop` ends the wait early. A list that does not come is read again; a copy that the
    /// router marks failed, or that left its list, ends the wait as `Failed`.
    pub async fn wait(
        &self,
        copy: Uuid,
        reached: fn(Status) -> bool,
        stop: impl Fn() -> bool,
    ) -> Waited {
        let end = tokio::time::Instant::now() + Duration::from_secs(WAIT_SECONDS);
        loop {
            if stop() {
                return Waited::Stopped;
            }
            let Ok(list) = tokio::time::timeout_at(end, self.statuses(false)).await else {
                return Waited::TimedOut;
            };
            let state = list.map(|list| {
                list.into_iter()
                    .find(|(id, _)| *id == copy)
                    .map(|(_, state)| state)
            });
            match state {
                Some(Some(state)) if reached(state) => return Waited::Reached,
                Some(Some(Status::Failed) | None) => return Waited::Failed,
                Some(Some(_)) | None => {}
            }
            if tokio::time::timeout_at(end, tokio::time::sleep(POLL))
                .await
                .is_err()
            {
                return Waited::TimedOut;
            }
        }
    }
}
