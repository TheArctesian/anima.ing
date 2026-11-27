//! The pure rules of a job (K10 Sections 4, 5 and 7).

use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::wire::{
    MAX_LOG_PART_BYTES, StopReason, WorkKind, WorkReport, WorkState, WorkType,
};

use super::{
    model::{Job, State},
    results,
};
use crate::{
    clock::lease::{self, Lost},
    ids::DeviceId,
};

pub(super) const STOPPED_REASON: &str = "machine stopped 3 times";

/// A JIT configuration is for one use.
pub(super) const RUNNER_LOST_REASON: &str = "machine stopped; a runner job has one attempt";

pub(super) const EXPIRED_REASON: &str = "expired";

/// How long after `limits.seconds` the hub ends an attempt that is not final (SF-011).
const OVERTIME: TimeDelta = TimeDelta::minutes(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    NotHolder,
    Malformed,
}

pub(super) fn claimed(job: &Job, device: DeviceId, now: DateTime<Utc>) -> Job {
    Job {
        state: State::Claimed,
        attempt: job.attempt.saturating_add(1),
        holder: Some(device),
        last_device: Some(device),
        lease_ends_at: Some(lease::end(now)),
        claimed_at: Some(now),
        stop: None,
        changed_at: now,
        ..job.clone()
    }
}

/// The diagram of K10 Section 7. The same state is a report that only holds the lease.
const fn permitted(from: State, to: WorkState) -> bool {
    use State::{Claimed, Finishing, Preparing, Running};
    matches!(
        (from, to),
        (
            Claimed,
            WorkState::Claimed | WorkState::Preparing | WorkState::Refused
        ) | (Preparing, WorkState::Preparing | WorkState::Running)
            | (Running, WorkState::Running | WorkState::Finishing)
            | (Finishing, WorkState::Finishing | WorkState::Done)
            | (
                Preparing | Running | Finishing,
                WorkState::Failed | WorkState::Stopped
            )
    )
}

/// The log part is not applied here: see [`log_fits`].
pub(super) fn reported(
    job: &Job,
    device: DeviceId,
    report: &WorkReport,
    now: DateTime<Utc>,
) -> Result<Job, Refusal> {
    let host = job.asked.kind == WorkKind::Host;
    let holds = job.holder == Some(device)
        && job.attempt == report.attempt
        && job.state.is_held()
        && if host {
            !overdue(job, now)
        } else {
            !lease::ended(job.lease_ends_at, now)
        };
    if !holds || !permitted(job.state, report.state) {
        return Err(Refusal::NotHolder);
    }
    if report.state.is_final() != report.result.is_some() || (host && report.log.is_some()) {
        return Err(Refusal::Malformed);
    }
    // Only codes of the fixed lists are kept: a result is never text of the machine.
    let result = match &report.result {
        Some(sent) if host => {
            Some(results::host(sent, report.state, job.asked.work_type).ok_or(Refusal::Malformed)?)
        }
        Some(sent) => Some(results::checked(sent).ok_or(Refusal::Malformed)?),
        None => None,
    };
    let Some(state) = reported_state(report.state, host) else {
        return Ok(refused(job, device, now));
    };
    let mut next = Job {
        state,
        changed_at: now,
        ..job.clone()
    };
    if state.is_final() {
        release(&mut next);
        next.reason = result.as_ref().and_then(|result| result.cause.clone());
        next.result = result;
        next.ended_at = Some(now);
    } else {
        next.lease_ends_at = Some(lease::end(now));
        if host && next.stop.is_none() && past_limit(job, now, 0) {
            next.stop = Some(StopReason::Expired);
        }
    }
    Ok(next)
}

pub(super) fn reported_state(state: WorkState, host: bool) -> Option<State> {
    if host && state == WorkState::Refused {
        Some(State::Failed)
    } else {
        State::reported(state)
    }
}

fn release(job: &mut Job) {
    job.holder = None;
    job.lease_ends_at = None;
    job.stop = None;
}

/// A refusal is not an attempt that was lost (K10 Section 5).
fn refused(job: &Job, device: DeviceId, now: DateTime<Utc>) -> Job {
    let mut next = queued_again(job, now);
    if !next.refused_by.contains(&device) {
        next.refused_by.push(device);
    }
    next
}

fn queued_again(job: &Job, now: DateTime<Utc>) -> Job {
    let mut next = Job {
        state: State::Queued,
        claimed_at: None,
        changed_at: now,
        ..job.clone()
    };
    release(&mut next);
    next
}

fn failed(job: &Job, reason: &str, now: DateTime<Utc>) -> Job {
    let mut next = Job {
        state: State::Failed,
        reason: Some(reason.to_owned()),
        ended_at: Some(now),
        changed_at: now,
        ..job.clone()
    };
    release(&mut next);
    next
}

/// A `build` fails the third time; a `runner` has no second attempt (K10 Section 7).
pub(super) fn lease_lost(job: &Job, now: DateTime<Utc>) -> Job {
    if job.asked.work_type == WorkType::Runner {
        return failed(job, RUNNER_LOST_REASON, now);
    }
    match lease::lost(job.lost) {
        Lost::Again(lost) => Job {
            lost,
            ..queued_again(job, now)
        },
        Lost::Failed(lost) => Job {
            lost,
            ..failed(job, STOPPED_REASON, now)
        },
    }
}

/// The answer to its claim was lost, so no lease counts as lost (SF-011).
pub(super) fn answer_lost(job: &Job, now: DateTime<Utc>) -> Job {
    if job.asked.kind == WorkKind::Host {
        orphan(job, now)
    } else {
        queued_again(job, now)
    }
}

pub(super) fn orphan(job: &Job, now: DateTime<Utc>) -> Job {
    failed(job, "orphan", now)
}

pub(super) fn overdue(job: &Job, now: DateTime<Utc>) -> bool {
    job.state.is_held() && past_limit(job, now, OVERTIME.num_seconds())
}

fn past_limit(job: &Job, now: DateTime<Utc>, grace: i64) -> bool {
    let limit = i64::try_from(job.asked.limits.seconds).unwrap_or(i64::MAX);
    let most = limit.saturating_add(grace);
    job.claimed_at
        .is_some_and(|since| now.signed_duration_since(since).num_seconds() >= most)
}

pub(super) fn expired(job: &Job, now: DateTime<Utc>) -> Job {
    let mut next = Job {
        state: State::Stopped,
        reason: Some(EXPIRED_REASON.to_owned()),
        ended_at: Some(now),
        changed_at: now,
        ..job.clone()
    };
    release(&mut next);
    next
}

/// A held job gets the stop in the next answer to its holder.
pub(super) fn stop_asked(job: &Job, reason: StopReason, now: DateTime<Utc>) -> Option<Job> {
    match job.state {
        State::Queued => Some(Job {
            state: State::Stopped,
            reason: Some(reason.as_str().to_owned()),
            ended_at: Some(now),
            changed_at: now,
            ..job.clone()
        }),
        state if state.is_held() => Some(Job {
            stop: job.stop.or(Some(reason)),
            ..job.clone()
        }),
        _ => None,
    }
}

pub(super) fn log_fits(job: &Job, from: u64, bytes: usize) -> bool {
    from == job.log_end && bytes <= MAX_LOG_PART_BYTES
}
