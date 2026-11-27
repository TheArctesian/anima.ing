//! The work routes of model work (K10 Section 2); the daemon asks, the hub never opens a connection.

use daemon_core::wire::{Claim, DaemonToken, Reported};
use serde_json::Value;

use super::answer::Answer;
use super::{Call, HubClient, json, read_json};

impl HubClient {
    /// The caller checks the item, as it knows the fixed lists.
    pub async fn claim_work(&self, token: &DaemonToken, claim: &Claim) -> Answer<Option<Value>> {
        let call = Call {
            route: "work/v1/claim",
            bearer: Some(token),
            body: json(claim),
            ok: &[204, 200],
        };
        self.post(call, |status, body| {
            if status == 204 {
                Some(None)
            } else {
                read_json::<Value>(body).map(Some)
            }
        })
        .await
    }

    pub async fn report_work(&self, token: &DaemonToken, report: &Value) -> Answer<Reported> {
        let call = Call {
            route: "work/v1/report",
            bearer: Some(token),
            body: json(report),
            ok: &[200],
        };
        self.post(call, |_, body| read_json(body)).await
    }
}

#[cfg(target_os = "linux")]
mod hosting;

#[cfg(target_os = "linux")]
pub use hosting::ApplicationStartError;

#[cfg(target_os = "linux")]
mod secrets;

#[cfg(target_os = "linux")]
pub(crate) use secrets::ReleasedSecret;
