//! Local cleanup proof precedes a final hub report.
use super::{Action, Coordinator, Entry, Error, Value};
use chrono::{DateTime, Utc};
use daemon_core::wire::{Cleanup, WorkResult, WorkState};
use uuid::Uuid;

impl Coordinator {
    pub(super) async fn release_if_terminal(
        &self,
        execution: Uuid,
        status: Value,
    ) -> Result<Value, Error> {
        let entry = self.owned(execution)?;
        let ready = status["phase"] == "ready" && entry.artifact_receipt.is_some();
        let failed = matches!(
            status["phase"].as_str(),
            Some("failed" | "expired" | "cancelled")
        );
        if !ready && !failed {
            return Ok(status);
        }
        // A busy cleanup owner must not interrupt lease reporting. Retry the same
        // receipt on the next observation until root registers or completes release.
        let recorded = self
            .call(
                entry,
                Action::Release {
                    execution,
                    receipt: entry.artifact_receipt.clone(),
                },
            )
            .await;
        Ok(recorded.unwrap_or(status))
    }
}

pub(super) fn completed(
    entry: &Entry,
    status: &Value,
) -> Result<Option<(WorkState, WorkResult)>, Error> {
    if status["phase"] != "released" || matches!(entry.reported, None | Some(WorkState::Claimed)) {
        return Ok(None);
    }
    let completion = match &entry.artifact_receipt {
        Some(receipt)
            if entry.reported == Some(WorkState::Finishing)
                && status["release_receipt"] == serde_json::to_value(receipt)? =>
        {
            (WorkState::Done, "succeeded", None)
        }
        None if status["release_receipt"].is_null() => match status["failure"].as_str() {
            Some("operator") => (WorkState::Stopped, "stopped", None),
            Some("lease_expired") => (WorkState::Stopped, "stopped", Some("orphan")),
            Some(cause @ ("secret_failed" | "limit_memory")) => {
                (WorkState::Failed, "failed", Some(cause))
            }
            Some(_) => (WorkState::Failed, "failed", Some("step_failed")),
            None => return Ok(None),
        },
        _ => return Err(Error::Unconfirmed),
    };
    let started = status["registered_at"]
        .as_str()
        .and_then(|time| DateTime::parse_from_rfc3339(time).ok())
        .ok_or(Error::Unconfirmed)?;
    let seconds =
        u64::try_from(Utc::now().signed_duration_since(started).num_seconds()).unwrap_or(0);
    let (state, outcome, cause) = completion;
    Ok(Some((
        state,
        WorkResult {
            outcome: outcome.into(),
            cause: cause.map(str::to_owned),
            seconds,
            health: None,
            cleanup: Cleanup {
                isolation: "deleted".into(),
                checkout: "deleted".into(),
                secrets: if daemon_core::source_jobs::private_checkout(&entry.job) {
                    "deleted"
                } else {
                    "none"
                }
                .into(),
                keychain: "none".into(),
            },
        },
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{home::Home, origin::Origin};
    use daemon_core::artifact::{Offer, Receipt};
    use serde_json::json;

    #[test]
    fn terminal_report_requires_completed_release_and_the_same_receipt() {
        let temp = tempfile::tempdir().unwrap();
        let home = Home::at(temp.path().join("daemon"));
        let device = Uuid::now_v7();
        let origin = Origin::parse("https://example.invalid").unwrap();
        let mut coordinator = Coordinator::open(&home, device, &origin).unwrap();
        let job = super::super::tests::job();
        let mut entry = coordinator
            .journal
            .reserve(device, &origin.as_text(), &job)
            .unwrap();
        entry.reported = Some(WorkState::Finishing);
        let receipt = Receipt {
            version: 1,
            id: Uuid::now_v7(),
            device,
            artifact: Offer {
                job: job.id,
                attempt: job.attempt,
                bytes: 1,
                archive_digest: format!("sha256:{}", "a".repeat(64)),
                manifest_digest: format!("sha256:{}", "b".repeat(64)),
            },
        };
        entry.artifact_receipt = Some(receipt.clone());
        let mut status =
            json!({"phase":"releasing","registered_at":Utc::now(), "release_receipt":receipt});
        assert!(completed(&entry, &status).unwrap().is_none());
        status["phase"] = json!("released");
        let (state, result) = completed(&entry, &status).unwrap().unwrap();
        assert_eq!(state, WorkState::Done);
        assert_eq!(result.cleanup.checkout, "deleted");
        status["release_receipt"]["id"] = json!(Uuid::now_v7());
        assert!(matches!(
            completed(&entry, &status),
            Err(Error::Unconfirmed)
        ));

        entry.artifact_receipt = None;
        entry.reported = Some(WorkState::Running);
        status = json!({"phase":"failed", "registered_at":Utc::now(), "failure":"limit_memory"});
        assert!(completed(&entry, &status).unwrap().is_none());
        status["phase"] = json!("released");
        let (state, result) = completed(&entry, &status).unwrap().unwrap();
        assert_eq!(state, WorkState::Failed);
        assert_eq!(result.cause.as_deref(), Some("limit_memory"));
        assert_eq!(result.cleanup.isolation, "deleted");
    }
}
