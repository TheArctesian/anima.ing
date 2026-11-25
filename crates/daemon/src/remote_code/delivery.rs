//! The authenticated daemon edge. The holder never receives the bearer.

use std::{path::Path, time::Duration};

use daemon_core::wire::{DaemonToken, RefusalCode, Reported, WorkState};
use serde_json::json;
use uuid::Uuid;

use crate::{
    config::origin::Reach,
    hub::{
        HubClient, HubError, TIMEOUT,
        answer::{Answer, Cause},
        remote_code::Claimed,
    },
};

use super::{
    broker::{Ask, DispatchReply, Phase, Reply},
    claim::Advertised,
    command::{Delivered, Receipt},
    wire::{ControlUpload, DispatchRequest, Evidence, LaunchReceipt},
};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(35);
const ERROR_DELAY: Duration = Duration::from_secs(5);

/// The two request slots have separate HTTP deadlines. One owns ordinary calls and the other
/// can wait for the full K10 long poll without blocking claim, evidence or result calls.
pub struct Delivery {
    hub: HubClient,
    commands: HubClient,
}

impl Delivery {
    /// Connect through the same verified origin as the daemon's K8 client.
    ///
    /// # Errors
    /// The origin or the local link cannot be configured.
    pub fn reaching(reach: Reach<'_>, own_socket: &Path) -> Result<Self, HubError> {
        Ok(Self {
            hub: HubClient::reaching(reach, own_socket, TIMEOUT)?,
            commands: HubClient::reaching(reach, own_socket, COMMAND_TIMEOUT)?,
        })
    }

    pub async fn claim(&self, token: &DaemonToken, claim: &Advertised<'_>) -> Answer<Claimed> {
        self.hub.claim_threads(token, claim).await
    }

    /// Relay only a request on the enrolled device's active original reservation.
    pub async fn relay(
        &self,
        token: &DaemonToken,
        (generation, original): (u64, &DispatchRequest),
        ask: Ask,
    ) -> Answer<Reply> {
        if !ask.is_on(generation) || ask.request() != original {
            return Answer::Failed(Cause::Unreadable);
        }
        match ask.phase() {
            Phase::Prepare => response(self.hub.prepare_thread(token, original).await, |report| {
                ask.reply(DispatchReply::Prepared {
                    stop: report.stop.map(|stop| stop.reason),
                })
            }),
            Phase::Authorize => response(
                self.hub.authorize_thread(token, original).await,
                |decision| ask.reply(DispatchReply::Dispatch { decision }),
            ),
        }
    }

    /// Ordinary reports do not renew or reopen a final attempt. After `NOT_HOLDER` the caller
    /// sends evidence on the original launch tuple; it leaves the local process running.
    pub async fn report_started(
        &self,
        token: &DaemonToken,
        started: &Evidence,
    ) -> Answer<Reported> {
        use super::wire::EvidenceOutcome;
        let EvidenceOutcome::Created { thread, .. } = started.outcome else {
            return Answer::Failed(Cause::Unreadable);
        };
        for state in [WorkState::Running, WorkState::Finishing] {
            let report = json!({
                "id": started.work, "attempt": started.attempt, "state": state,
                "log": null, "result": null
            });
            let result = self.hub.report_work(token, &report).await;
            if !matches!(result, Answer::Accepted(_)) {
                return result;
            }
        }
        let report = json!({
            "id": started.work, "attempt": started.attempt, "state": WorkState::Done,
            "log": null, "result": {
                "outcome": "succeeded", "cause": null, "seconds": 0, "thread": thread,
                "cleanup": { "isolation": "none", "checkout": "none", "secrets": "none", "keychain": "none" }
            }
        });
        self.hub.report_work(token, &report).await
    }

    pub async fn evidence(
        &self,
        token: &DaemonToken,
        evidence: &Evidence,
    ) -> Answer<LaunchReceipt> {
        self.hub.thread_launch_evidence(token, evidence).await
    }

    /// Preserve the dedicated command slot across an empty long poll.
    pub async fn poll(&self, token: &DaemonToken, device: Uuid) -> Answer<Vec<Delivered>> {
        self.commands.poll_thread_commands(token, device).await
    }

    pub async fn receipt(&self, token: &DaemonToken, receipt: &Receipt) -> Answer<()> {
        self.hub.report_thread_command(token, receipt).await
    }

    pub async fn control(&self, token: &DaemonToken, offer: &ControlUpload<'_>) -> Answer<()> {
        self.hub.upload_thread_control(token, offer).await
    }
}

/// A command poll may immediately repeat on success. K8 backoff is supplied by its caller.
#[must_use]
pub fn next_poll<T>(answer: &Answer<T>, backoff: Duration) -> Option<Duration> {
    match answer {
        Answer::Accepted(_) => Some(Duration::ZERO),
        Answer::Refused(RefusalCode::TokenRefused | RefusalCode::WrongDevice) => None,
        Answer::Failed(Cause::Status(429 | 500..=599))
        | Answer::Refused(RefusalCode::Unavailable) => Some(backoff),
        _ => Some(ERROR_DELAY),
    }
}

fn response<A, B>(answer: Answer<A>, read: impl FnOnce(A) -> B) -> Answer<B> {
    match answer {
        Answer::Accepted(value) => Answer::Accepted(read(value)),
        Answer::Refused(code) => Answer::Refused(code),
        Answer::Failed(cause) => Answer::Failed(cause),
    }
}
