use super::{Action, Coordinator, Entry, Error, Value};
use daemon_core::source_jobs::Grant;
use uuid::Uuid;

impl Entry {
    pub(super) fn may_run(&self) -> bool {
        !self.settled
            && !self.stop_requested
            && !self
                .report
                .as_ref()
                .is_some_and(|report| report.state.is_final())
    }

    fn start(&mut self) -> Result<(), Error> {
        if !self.may_run() {
            return Err(Error::Conflict);
        }
        self.start_requested = true;
        Ok(())
    }

    fn renew(&mut self, grant: Grant) -> Result<(), Error> {
        let until = self.renewal.map_or(self.job.lease.until, |old| old.until);
        if !self.may_run()
            || grant.job != self.job.id
            || grant.attempt != self.job.attempt
            || grant.until < until
        {
            return Err(Error::Conflict);
        }
        self.renewal = Some(grant);
        Ok(())
    }
}

impl Coordinator {
    /// Pending execution identities from this enrollment only; never root discovery.
    ///
    /// # Errors
    /// An ambiguous storage failure requires reopening the journal.
    pub fn executions(&self) -> Result<Vec<Uuid>, Error> {
        self.journal.ready()?;
        Ok(self
            .journal
            .entries
            .iter()
            .filter(|entry| self.belongs(entry) && !entry.settled)
            .map(|entry| entry.execution)
            .collect())
    }

    /// Persists start intent before asking the root owner to dispatch it.
    ///
    /// # Errors
    /// Unknown, stopped or terminal intent, storage failure or an unconfirmed reply.
    pub async fn start(&mut self, execution: Uuid) -> Result<(), Error> {
        let mut entry = self.owned(execution)?.clone();
        entry.start()?;
        self.journal.replace(entry.clone())?;
        self.call(&entry, Action::Start { execution })
            .await
            .map(drop)
    }

    /// Persists the hub's exact lease grant before forwarding it to the root owner.
    ///
    /// # Errors
    /// Mismatched/regressing grant, stopped intent, storage failure or unconfirmed reply.
    pub async fn renew(&mut self, execution: Uuid, lease: Grant) -> Result<(), Error> {
        let mut entry = self.owned(execution)?.clone();
        entry.renew(lease)?;
        self.journal.replace(entry.clone())?;
        self.call(&entry, Action::Renew { execution, lease })
            .await
            .map(drop)
    }

    /// Stop intent survives disconnects and prevents any later start or renewal.
    ///
    /// # Errors
    /// Unknown/settled intent, storage failure or an unconfirmed root reply.
    pub async fn cancel(&mut self, execution: Uuid) -> Result<(), Error> {
        let mut entry = self.owned(execution)?.clone();
        if entry.settled {
            return Err(Error::Conflict);
        }
        entry.stop_requested = true;
        self.journal.replace(entry.clone())?;
        self.call(&entry, Action::Cancel { execution })
            .await
            .map(drop)
    }

    /// Observes only a locally owned execution, checking the full immutable job identity.
    ///
    /// # Errors
    /// Unknown intent, unavailable owner or mismatched reply.
    pub async fn status(&self, execution: Uuid) -> Result<Value, Error> {
        self.call(self.owned(execution)?, Action::Status { execution })
            .await
    }

    /// Replays durable lifecycle intent without adopting or starting unknown work.
    ///
    /// # Errors
    /// Unknown intent, unavailable owner, refused renewal or mismatched reply.
    pub async fn reconcile(&self, execution: Uuid) -> Result<Value, Error> {
        let entry = self.owned(execution)?;
        let mut status = match self.status(execution).await {
            Ok(status) => status,
            Err(Error::Unconfirmed) if !entry.start_requested && entry.may_run() => {
                return self
                    .call(
                        entry,
                        Action::Register {
                            execution,
                            item: serde_json::to_value(&entry.job)?,
                        },
                    )
                    .await;
            }
            Err(error) => return Err(error),
        };
        if entry.settled {
            return Ok(status);
        }
        if entry.stop_requested {
            return self.call(entry, Action::Cancel { execution }).await;
        }
        if !entry.may_run() {
            return Ok(status);
        }
        let phase = status["phase"].as_str().ok_or(Error::Unconfirmed)?;
        let renewable = matches!(
            phase,
            "registered"
                | "starting"
                | "preparing"
                | "awaiting-checkout-credential"
                | "checking-out"
                | "capturing"
                | "building"
                | "finishing"
        );
        if renewable && let Some(lease) = entry.renewal {
            status = self.call(entry, Action::Renew { execution, lease }).await?;
        }
        if status["phase"] == "registered" && entry.start_requested && entry.may_run() {
            return self.call(entry, Action::Start { execution }).await;
        }
        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_and_monotonic_grants_survive_journal_recovery() {
        let temporary = tempfile::tempdir().unwrap();
        let home = crate::config::home::Home::at(temporary.path().join("daemon"));
        let device = Uuid::now_v7();
        let origin = crate::config::origin::Origin::parse("https://example.invalid").unwrap();
        let job = super::super::tests::job();
        let mut owner = Coordinator::open(&home, device, &origin).unwrap();
        let mut entry = owner
            .journal
            .reserve(device, &origin.as_text(), &job)
            .unwrap();
        entry.start().unwrap();
        let lease = Grant {
            job: job.id,
            attempt: job.attempt,
            until: job.lease.until + chrono::TimeDelta::seconds(30),
        };
        entry.renew(lease).unwrap();
        assert!(
            entry
                .renew(Grant {
                    attempt: 2,
                    ..lease
                })
                .is_err()
        );
        assert!(
            entry
                .renew(Grant {
                    until: job.lease.until,
                    ..lease
                })
                .is_err()
        );
        owner.journal.replace(entry.clone()).unwrap();
        entry.stop_requested = true;
        owner.journal.replace(entry.clone()).unwrap();
        drop(owner);
        let owner = Coordinator::open(&home, device, &origin).unwrap();
        let mut restored = owner.owned(entry.execution).unwrap().clone();
        assert!(restored.start_requested && restored.stop_requested);
        assert_eq!(restored.renewal.unwrap().until, lease.until);
        assert!(restored.start().is_err());
        assert!(restored.renew(lease).is_err());
        assert_eq!(owner.executions().unwrap(), vec![entry.execution]);
    }
}
