use super::{Coordinator, Error};
use daemon_core::{
    source_jobs::Grant,
    wire::{Reported, WorkReport, WorkState},
};
use uuid::Uuid;

impl Coordinator {
    /// Replays a pending report or persists the next legal nonterminal progress report.
    /// Artifact readiness alone never reports completed delivery or cleanup.
    ///
    /// # Errors
    /// Unknown execution/phase, settled intent, root unavailability or storage failure.
    pub async fn prepare_report(&mut self, execution: Uuid) -> Result<WorkReport, Error> {
        let entry = self.owned(execution)?;
        if entry.settled {
            return Err(Error::Conflict);
        }
        if let Some(report) = &entry.report {
            return Ok(report.clone());
        }
        let status = self.reconcile(execution).await?;
        let status = self.release_if_terminal(execution, status).await?;
        let entry = self.owned(execution)?;
        let phase = status["phase"].as_str().ok_or(Error::Unconfirmed)?;
        let previous = entry.reported.unwrap_or(WorkState::Claimed);
        let (state, result) = match super::terminal::completed(entry, &status)? {
            Some((state, result)) => (state, Some(result)),
            None => (progress(previous, phase)?, None),
        };
        let report = WorkReport {
            id: entry.job.id,
            attempt: entry.job.attempt,
            state,
            log: None,
            result,
            progress: None,
        };
        self.report(execution, report.clone())?;
        Ok(report)
    }

    /// Atomically saves the hub's decision while clearing the exact accepted report.
    /// Stop wins over renewal; an acknowledged terminal report retains its tombstone.
    ///
    /// # Errors
    /// Stale report, regressing lease, another enrollment or storage failure.
    pub fn acknowledge(
        &mut self,
        execution: Uuid,
        report: &WorkReport,
        response: Reported,
    ) -> Result<(), Error> {
        let mut entry = self.owned(execution)?.clone();
        if entry.report.as_ref() != Some(report) {
            return Err(Error::Conflict);
        }
        if !report.state.is_final() {
            if response.stop.is_some() {
                entry.stop_requested = true;
            } else if !entry.stop_requested {
                let until = entry
                    .renewal
                    .map_or(entry.job.lease.until, |grant| grant.until);
                if response.lease.until < until {
                    return Err(Error::Conflict);
                }
                entry.renewal = Some(Grant {
                    job: entry.job.id,
                    attempt: entry.job.attempt,
                    until: response.lease.until,
                });
            }
        }
        entry.settled = report.state.is_final();
        entry.reported = Some(report.state);
        entry.report = None;
        self.journal.replace(entry)
    }
}

fn progress(previous: WorkState, phase: &str) -> Result<WorkState, Error> {
    let target = match phase {
        "registered" => 0,
        "starting"
        | "preparing"
        | "awaiting-checkout-credential"
        | "checking-out"
        | "capturing"
        | "cancelling"
        | "failed"
        | "expired"
        | "cancelled"
        | "releasing"
        | "released" => 1,
        "building" => 2,
        "finishing" | "ready" => 3,
        _ => return Err(Error::Unconfirmed),
    };
    let (stage, next) = match previous {
        WorkState::Claimed => (0, WorkState::Preparing),
        WorkState::Preparing => (1, WorkState::Running),
        WorkState::Running => (2, WorkState::Finishing),
        WorkState::Finishing => (3, WorkState::Finishing),
        _ => return Err(Error::Conflict),
    };
    Ok(if target > stage { next } else { previous })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{home::Home, origin::Origin};
    use daemon_core::wire::{Lease, StopOrder, StopReason, WorkState};

    #[test]
    fn ready_work_reports_each_required_stage_but_never_invents_terminal_cleanup() {
        let mut state = WorkState::Claimed;
        for expected in [
            WorkState::Preparing,
            WorkState::Running,
            WorkState::Finishing,
            WorkState::Finishing,
        ] {
            state = progress(state, "ready").unwrap();
            assert_eq!(state, expected);
        }
        assert_eq!(
            progress(WorkState::Running, "registered").unwrap(),
            WorkState::Running
        );
        for phase in ["failed", "expired", "cancelled", "released"] {
            assert_eq!(
                progress(WorkState::Claimed, phase).unwrap(),
                WorkState::Preparing
            );
            assert_eq!(
                progress(WorkState::Preparing, phase).unwrap(),
                WorkState::Preparing
            );
        }
        assert!(progress(WorkState::Claimed, "unknown").is_err());
        assert!(progress(WorkState::Done, "ready").is_err());
    }

    #[test]
    fn accepting_a_report_saves_renewal_or_stop_before_forgetting_delivery() {
        let temporary = tempfile::tempdir().unwrap();
        let home = Home::at(temporary.path().join("daemon"));
        let device = Uuid::now_v7();
        let origin = Origin::parse("https://example.invalid").unwrap();
        let job = super::super::tests::job();
        let mut owner = Coordinator::open(&home, device, &origin).unwrap();
        let execution = owner
            .journal
            .reserve(device, &origin.as_text(), &job)
            .unwrap()
            .execution;
        let report = WorkReport {
            progress: None,
            id: job.id,
            attempt: job.attempt,
            state: WorkState::Running,
            log: None,
            result: None,
        };
        owner.report(execution, report.clone()).unwrap();
        let response = Reported {
            lease: Lease {
                until: job.lease.until + chrono::TimeDelta::seconds(30),
            },
            stop: None,
            log_end: None,
        };
        owner.acknowledge(execution, &report, response).unwrap();
        drop(owner);
        let mut owner = Coordinator::open(&home, device, &origin).unwrap();
        assert_eq!(owner.pending().unwrap(), []);
        assert_eq!(
            owner.owned(execution).unwrap().renewal.unwrap().until,
            response.lease.until
        );
        owner.report(execution, report.clone()).unwrap();
        let old = Reported {
            lease: job.lease,
            ..response
        };
        assert!(owner.acknowledge(execution, &report, old).is_err());
        assert_eq!(owner.pending().unwrap(), vec![(execution, report.clone())]);
        let stop = Reported {
            stop: Some(StopOrder {
                reason: StopReason::Operator,
            }),
            ..old
        };
        owner.acknowledge(execution, &report, stop).unwrap();
        drop(owner);
        let mut owner = Coordinator::open(&home, device, &origin).unwrap();
        assert_eq!(owner.pending().unwrap(), []);
        assert!(owner.owned(execution).unwrap().stop_requested);
        owner.report(execution, report.clone()).unwrap();
        owner.acknowledge(execution, &report, response).unwrap();
        assert!(owner.owned(execution).unwrap().stop_requested);
    }
}
