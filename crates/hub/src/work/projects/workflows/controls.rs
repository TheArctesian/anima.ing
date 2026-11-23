//! Whether a control may be used on a run (PRJ-034; K-E1 Section 13.3; decisions item 10; CQ-021).
//! A run of a fork is not run again from Anima, but it can be cancelled because that stops code (PRO-013).

use chrono::{DateTime, TimeDelta, Utc};
use provider_github::live::runs::Run;
use source_exchange::CommitSha;

use super::views::{WorkflowConclusion, WorkflowRunStatus, conclusion, run_status};

/// GitHub runs a run again only this many days after it started.
const RERUN_DAYS: i64 = 30;
/// GitHub runs a run again at most this many times, whole or in part.
const MOST_ATTEMPTS: u32 = 50;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Rerun,
    RerunFailedJobs,
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunState {
    pub status: WorkflowRunStatus,
    pub conclusion: Option<WorkflowConclusion>,
    pub head_sha: String,
    pub from_fork: bool,
    pub created_at: DateTime<Utc>,
    pub attempt: u32,
}

impl RunState {
    #[must_use]
    pub fn of(run: &Run) -> Self {
        let status = run_status(run.status);
        Self {
            status,
            conclusion: run
                .conclusion
                .filter(|_| status == WorkflowRunStatus::Completed)
                .map(conclusion),
            head_sha: run.head_sha.to_ascii_lowercase(),
            from_fork: run.from_fork,
            created_at: run.created_at,
            attempt: run.attempt,
        }
    }
}

/// Why a control may not be used on a run now. Fixed words, the GraphQL message (`CONFLICT`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Barred {
    #[error("The run's commit is not the one you saw. Read the run again.")]
    HeadMoved,
    #[error("The run has not finished. Only a finished run can run again.")]
    NotFinished,
    #[error("The run has no failed, cancelled or timed out jobs to run again.")]
    NoFailedJobs,
    #[error("The run is from a fork. Anima does not run it again (PRO-013).")]
    FromFork,
    #[error("The run started more than 30 days ago. GitHub runs it again only within 30 days.")]
    TooOld,
    #[error("The run has run 50 times. GitHub runs it again at most 50 times.")]
    TooManyAttempts,
    #[error("The run has finished. There is nothing to cancel.")]
    Finished,
}

/// # Errors
/// The [`Barred`] reason.
pub fn decide(
    control: Control,
    run: &RunState,
    expected: &CommitSha,
    now: DateTime<Utc>,
) -> Result<(), Barred> {
    if run.head_sha != expected.as_str() {
        return Err(Barred::HeadMoved);
    }
    let completed = run.status == WorkflowRunStatus::Completed;
    match control {
        Control::Cancel => {
            if completed {
                Err(Barred::Finished)
            } else {
                Ok(())
            }
        }
        Control::Rerun => again(run, completed, now),
        Control::RerunFailedJobs => {
            again(run, completed, now)?;
            let failing = matches!(
                run.conclusion,
                Some(
                    WorkflowConclusion::Failure
                        | WorkflowConclusion::Cancelled
                        | WorkflowConclusion::TimedOut
                )
            );
            if failing {
                Ok(())
            } else {
                Err(Barred::NoFailedJobs)
            }
        }
    }
}

fn again(run: &RunState, completed: bool, now: DateTime<Utc>) -> Result<(), Barred> {
    if !completed {
        Err(Barred::NotFinished)
    } else if run.from_fork {
        Err(Barred::FromFork)
    } else if now - run.created_at > TimeDelta::days(RERUN_DAYS) {
        Err(Barred::TooOld)
    } else if run.attempt >= MOST_ATTEMPTS {
        Err(Barred::TooManyAttempts)
    } else {
        Ok(())
    }
}
