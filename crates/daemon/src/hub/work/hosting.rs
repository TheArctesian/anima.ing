//! The running acknowledgement fences the start of an exact prepared host attempt.
use daemon_core::wire::{DaemonToken, RefusalCode, StopReason, WorkReport, WorkState};

use super::{Answer, HubClient};
use crate::{
    applications::{self, Outcome, Prepared},
    hub::answer::Cause,
};

#[derive(Debug, thiserror::Error)]
pub enum ApplicationStartError {
    #[error("the hub refused the running report ({0:?})")]
    Refused(RefusalCode),
    #[error("the running report was not acknowledged: {0}")]
    Unconfirmed(Cause),
    #[error("the hub stopped the attempt ({0:?})")]
    Stopped(StopReason),
    #[error(transparent)]
    Local(#[from] applications::Error),
}

impl HubClient {
    /// Start the received host item once its preparing report has been accepted.
    ///
    /// # Errors
    /// The hub did not authorize this attempt to run, or the owner socket failed.
    /// After a local transport failure the caller must never resend the start.
    pub async fn start_application(
        &self,
        token: &DaemonToken,
        prepared: Prepared,
    ) -> Result<Outcome, ApplicationStartError> {
        let item = prepared.item();
        let body = serde_json::to_value(WorkReport {
            id: item.id,
            attempt: item.attempt,
            state: WorkState::Running,
            log: None,
            result: None,
        })
        .map_err(|_| ApplicationStartError::Unconfirmed(Cause::Unreadable))?;
        let acknowledgement = match self.report_work(token, &body).await {
            Answer::Accepted(acknowledgement) => acknowledgement,
            Answer::Refused(code) => return Err(ApplicationStartError::Refused(code)),
            Answer::Failed(cause) => return Err(ApplicationStartError::Unconfirmed(cause)),
        };
        if let Some(stop) = acknowledgement.stop {
            return Err(ApplicationStartError::Stopped(stop.reason));
        }
        Ok(prepared.start(acknowledgement).await?)
    }
}
