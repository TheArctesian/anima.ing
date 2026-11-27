//! A private source attempt asks the Hub for its credential at most once.
use chrono::{DateTime, Utc};
use daemon_core::{
    source_jobs::{
        Action, Attempt, MAX_REPLY_BYTES, MAX_REQUEST_BYTES, Request, SOCKET, private_checkout,
    },
    wire::{Held, WorkState},
};
use serde_json::Value;
use uuid::Uuid;

use crate::{hub::work::ReleasedSecret, runtime::owner_socket};

use super::{Coordinator, Error};

pub(crate) struct Checkout {
    pub execution: Uuid,
    pub held: Held,
    pub fetch: bool,
    device: Uuid,
    origin: String,
}

impl Coordinator {
    pub(crate) fn checkout(
        &mut self,
        execution: Uuid,
        status: &Value,
        now: DateTime<Utc>,
    ) -> Result<Option<Checkout>, Error> {
        let mut entry = self.owned(execution)?.clone();
        if status["phase"] != "awaiting-checkout-credential"
            || !entry.may_run()
            || !private_checkout(&entry.job)
            || !matches!(
                entry.reported,
                Some(WorkState::Preparing | WorkState::Running)
            )
            || entry.renewal.is_none_or(|lease| lease.until <= now)
        {
            return Ok(None);
        }
        let fetch = entry.checkout_requested_at.is_none();
        let held = Held {
            id: entry.job.id,
            attempt: entry.job.attempt,
        };
        if fetch {
            entry.checkout_requested_at = Some(now);
            self.journal.replace(entry)?;
        }
        Ok(Some(Checkout {
            execution,
            held,
            fetch,
            device: self.device,
            origin: self.origin.clone(),
        }))
    }
}

impl Checkout {
    pub(crate) async fn deliver(self, credential: Option<ReleasedSecret>) {
        let credential = credential.filter(ReleasedSecret::live);
        let request = Request {
            device: self.device,
            origin: self.origin,
            action: Action::Checkout {
                identity: Attempt {
                    execution: self.execution,
                    job: self.held.id,
                    attempt: self.held.attempt,
                },
                expires_at: credential.as_ref().map(ReleasedSecret::expires_at),
            },
        };
        let _ = owner_socket::call_input(
            SOCKET,
            &request,
            (MAX_REQUEST_BYTES, MAX_REPLY_BYTES),
            credential.as_ref().map(ReleasedSecret::bytes),
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{home::Home, origin::Origin};
    use daemon_core::wire::{Lease, REPOSITORY_TOKEN, Reported, WorkJob, WorkReport};
    use serde_json::json;

    #[test]
    fn an_interrupted_private_checkout_refuses_to_fetch_the_same_attempt_after_restart() {
        let temporary = tempfile::tempdir().unwrap();
        let home = Home::at(temporary.path().join("daemon"));
        let device = Uuid::now_v7();
        let origin = Origin::parse("https://example.invalid").unwrap();
        let mut coordinator = Coordinator::open(&home, device, &origin).unwrap();
        let mut job = super::super::tests::job();
        job.project = Some(Uuid::now_v7());
        job.secrets = vec![REPOSITORY_TOKEN.to_owned()];
        let now = job.lease.until - chrono::TimeDelta::seconds(10);
        let execution = coordinator
            .journal
            .reserve(device, &origin.as_text(), &job)
            .unwrap()
            .execution;
        let status = json!({"phase":"awaiting-checkout-credential"});
        assert!(
            coordinator
                .checkout(execution, &status, now)
                .unwrap()
                .is_none()
        );
        acknowledge_preparing(&mut coordinator, execution, &job);
        let first = coordinator
            .checkout(execution, &status, now)
            .unwrap()
            .unwrap();
        assert!(first.fetch);
        assert_eq!((first.held.id, first.held.attempt), (job.id, job.attempt));
        drop(coordinator);

        let mut recovered = Coordinator::open(&home, device, &origin).unwrap();
        let failed = recovered
            .checkout(execution, &status, now)
            .unwrap()
            .unwrap();
        assert!(!failed.fetch);
        assert_eq!(failed.execution, first.execution);
        let awaiting_cleanup =
            json!({"phase":"failed","failure":"secret_failed","registered_at":now});
        let entry = recovered.owned(execution).unwrap();
        assert!(
            super::super::terminal::completed(entry, &awaiting_cleanup)
                .unwrap()
                .is_none()
        );
        let mut cleaned = awaiting_cleanup;
        cleaned["phase"] = json!("released");
        let (state, result) = super::super::terminal::completed(entry, &cleaned)
            .unwrap()
            .unwrap();
        assert_eq!(
            (state, result.cause.as_deref()),
            (WorkState::Failed, Some("secret_failed"))
        );
        assert_eq!(result.cleanup.secrets, "deleted");
    }

    fn acknowledge_preparing(coordinator: &mut Coordinator, execution: Uuid, job: &WorkJob) {
        let report = WorkReport {
            id: job.id,
            attempt: job.attempt,
            state: WorkState::Preparing,
            result: None,
            log: None,
        };
        coordinator.report(execution, report.clone()).unwrap();
        coordinator
            .acknowledge(
                execution,
                &report,
                Reported {
                    lease: Lease {
                        until: job.lease.until,
                    },
                    stop: None,
                    log_end: None,
                },
            )
            .unwrap();
    }
}
