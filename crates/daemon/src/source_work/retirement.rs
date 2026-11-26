//! Retire only after root proves the attempt cannot run or still owns its output.
use super::journal::Disposition;
use super::{
    Action, Coordinator, Error, MAX_REPLY_BYTES, MAX_REQUEST_BYTES, Request, SOCKET, Value,
};
use chrono::{DateTime, Utc};
use daemon_core::source_jobs::Attempt;
use uuid::Uuid;

impl Coordinator {
    /// Reclaim one settled identity, only after the root owner confirms retirement.
    pub(crate) async fn prune(
        &mut self,
        cursor: &mut usize,
        now: DateTime<Utc>,
    ) -> Result<(), Error> {
        let candidates: Vec<_> = self
            .journal
            .entries
            .iter()
            .filter(|entry| self.belongs(entry) && entry.settled && grant_until(entry) <= now)
            .collect();
        if candidates.is_empty() {
            return Ok(());
        }
        *cursor %= candidates.len();
        let entry = candidates[*cursor];
        let identity = Attempt {
            execution: entry.execution,
            job: entry.job.id,
            attempt: entry.job.attempt,
        };
        let request = Request {
            device: self.device,
            origin: self.origin.clone(),
            action: Action::Retire { identity },
        };
        let reply = crate::runtime::owner_socket::call(
            SOCKET,
            &request,
            (MAX_REQUEST_BYTES, MAX_REPLY_BYTES),
        )
        .await;
        if reply.and_then(|value| serde_json::from_value::<Attempt>(value).ok()) == Some(identity) {
            self.journal.forget(identity.execution)?;
        } else {
            *cursor += 1;
        }
        Ok(())
    }

    pub(crate) fn note_lost(&mut self, execution: Uuid) -> Result<(), Error> {
        let mut entry = self.owned(execution)?.clone();
        entry.stop_requested = true;
        entry.disposition = Disposition::Lost;
        self.journal.replace(entry)
    }

    /// Returns true when this tick must not send another report for the attempt.
    pub(crate) async fn reconcile_retirement(
        &mut self,
        execution: Uuid,
        now: DateTime<Utc>,
    ) -> Result<bool, Error> {
        let entry = self.owned(execution)?;
        let until = grant_until(entry);
        if !entry.start_requested
            && (entry.disposition == Disposition::Lost || until <= now)
            && self.unregistered(execution).await
        {
            self.settle_lost(execution)?;
            return Ok(true);
        }
        if entry.disposition != Disposition::Lost {
            return Ok(false);
        }
        if let Ok(status) = self.reconcile(execution).await {
            let status = self.release_if_terminal(execution, status).await?;
            // A completed, undelivered artifact stays under the storage owner's control.
            if status["phase"] == "released"
                || (status["phase"] == "ready" && entry.artifact_receipt.is_none())
            {
                self.settle_lost(execution)?;
            }
        }
        Ok(true)
    }

    fn settle_lost(&mut self, execution: Uuid) -> Result<(), Error> {
        let mut entry = self.owned(execution)?.clone();
        entry.stop_requested = true;
        entry.disposition = Disposition::Lost;
        entry.settled = true;
        entry.report = None;
        self.journal.replace(entry)
    }

    async fn unregistered(&self, execution: Uuid) -> bool {
        let request = Request {
            device: self.device,
            origin: self.origin.clone(),
            action: Action::List,
        };
        crate::runtime::owner_socket::call(SOCKET, &request, (MAX_REQUEST_BYTES, MAX_REPLY_BYTES))
            .await
            .is_some_and(|value| absent(&value, execution))
    }
}

fn grant_until(entry: &super::journal::Entry) -> DateTime<Utc> {
    entry
        .renewal
        .map_or(entry.job.lease.until, |grant| grant.until)
}

fn absent(value: &Value, execution: Uuid) -> bool {
    value["truncated"] == false
        && value["executions"].as_array().is_some_and(|entries| {
            entries.iter().all(|entry| {
                entry["execution"]
                    .as_str()
                    .and_then(|id| Uuid::parse_str(id).ok())
                    .is_some_and(|id| id != execution)
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn absence_requires_a_complete_identified_broker_inventory() {
        let execution = Uuid::now_v7();
        let inventory = |entries| json!({"truncated":false,"executions":entries});
        assert!(absent(&inventory(json!([])), execution));
        assert!(!absent(
            &inventory(json!([{"execution":execution}])),
            execution
        ));
        assert!(!absent(
            &inventory(json!([{"execution":"unknown"}])),
            execution
        ));
        assert!(!absent(
            &json!({"truncated":true,"executions":[]}),
            execution
        ));
    }
}
