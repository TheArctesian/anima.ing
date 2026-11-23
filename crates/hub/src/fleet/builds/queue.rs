//! The pure job rules (BLD-009); time comes in as a parameter (CQ-021).

use chrono::{DateTime, TimeDelta, Utc};

use super::{
    log_lines,
    model::{Arch, BuildJob, Builder, JobState, Os, Platform, Report, Target},
};
use crate::{
    clock::lease::{self, Lost},
    ids::BuilderId,
};

pub(super) const ONLINE_WITHIN: TimeDelta = TimeDelta::seconds(60);

/// The reason of a job whose lease ended [`lease::MAX_LOST`] times (BLD-006).
pub(super) const STOPPED_REASON: &str = "builder stopped 3 times";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Refusal {
    NotHolder,
    /// The job goes back to the queue (BLD-006).
    LeaseEnded,
    /// The diagram does not permit this change (BLD-010), or the job left the queue before a cancel (BLD-008).
    Change {
        from: JobState,
        to: JobState,
    },
}

/// The diagram of Section 3.10.1.
pub(super) const fn permitted(from: JobState, to: JobState) -> bool {
    use JobState::{Building, Cancelled, Claimed, Failed, Published, Queued, Uploading};
    matches!(
        (from, to),
        (Queued, Claimed | Cancelled)
            | (Claimed, Building | Failed | Queued)
            | (Building, Uploading | Failed | Queued)
            | (Uploading, Published | Failed | Queued)
    )
}

pub(super) fn lease_end(now: DateTime<Utc>) -> DateTime<Utc> {
    lease::end(now)
}

pub(super) fn lease_ended(job: &BuildJob, now: DateTime<Utc>) -> bool {
    job.state.is_leased() && lease::ended(job.lease_ends_at, now)
}

pub(super) const fn can_build(platform: Platform, target: Target) -> bool {
    matches!(
        (target, platform.os, platform.arch),
        (Target::LinuxX86_64, Os::Linux, Arch::X86_64)
            | (Target::DarwinUniversal, Os::Macos, Arch::Aarch64)
            | (Target::WindowsX86_64, Os::Windows, Arch::X86_64)
    )
}

pub(super) fn buildable(platform: Platform) -> Vec<Target> {
    Target::ALL
        .into_iter()
        .filter(|target| can_build(platform, *target))
        .collect()
}

pub(super) fn online(last_seen: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool {
    last_seen.is_some_and(|seen| now - seen <= ONLINE_WITHIN)
}

fn entered(job: &BuildJob, to: JobState, now: DateTime<Utc>) -> BuildJob {
    let mut next = job.clone();
    next.state = to;
    next.times.changed = now;
    let slot = match to {
        JobState::Queued => {
            next.times.queued = now;
            return next;
        }
        JobState::Claimed => &mut next.times.claimed,
        JobState::Building => &mut next.times.building,
        JobState::Uploading => &mut next.times.uploading,
        JobState::Published => &mut next.times.published,
        JobState::Failed => &mut next.times.failed,
        JobState::Cancelled => &mut next.times.cancelled,
    };
    *slot = Some(now);
    next
}

pub(super) fn claim(job: &BuildJob, builder: &Builder, now: DateTime<Utc>) -> BuildJob {
    let mut next = entered(job, JobState::Claimed, now);
    next.builder = Some(builder.id);
    next.builder_name = Some(builder.name.clone());
    next.lease_ends_at = Some(lease_end(now));
    next
}

/// A report of the current state only extends the lease; publishing is the hub's, never a builder's (BLD-005, BLD-010, BLD-066).
pub(super) fn report(
    job: &BuildJob,
    holder: BuilderId,
    report: &Report,
    now: DateTime<Utc>,
) -> Result<BuildJob, Refusal> {
    if job.builder != Some(holder) || !job.state.is_leased() {
        return Err(Refusal::NotHolder);
    }
    if lease_ended(job, now) {
        return Err(Refusal::LeaseEnded);
    }
    let to = report.state.unwrap_or(job.state);
    let reportable = matches!(
        to,
        JobState::Building | JobState::Uploading | JobState::Failed
    );
    let mut next = if to == job.state {
        job.clone()
    } else if reportable && permitted(job.state, to) {
        entered(job, to, now)
    } else {
        return Err(Refusal::Change {
            from: job.state,
            to,
        });
    };
    if to == JobState::Failed {
        next.lease_ends_at = None;
        next.failure_reason = Some(log_lines::reason(report.reason.as_deref()));
    } else {
        next.lease_ends_at = Some(lease_end(now));
    }
    Ok(next)
}

pub(super) fn expire(job: &BuildJob, now: DateTime<Utc>) -> BuildJob {
    let (mut next, attempts) = match lease::lost(job.attempts) {
        Lost::Failed(attempts) => {
            let mut failed = entered(job, JobState::Failed, now);
            failed.failure_reason = Some(STOPPED_REASON.to_owned());
            (failed, attempts)
        }
        Lost::Again(attempts) => (entered(job, JobState::Queued, now), attempts),
    };
    next.attempts = attempts;
    next.builder = None;
    next.lease_ends_at = None;
    next
}

pub(super) fn cancel(job: &BuildJob, now: DateTime<Utc>) -> Result<BuildJob, Refusal> {
    if job.state == JobState::Queued {
        Ok(entered(job, JobState::Cancelled, now))
    } else {
        Err(Refusal::Change {
            from: job.state,
            to: JobState::Cancelled,
        })
    }
}

pub(super) fn published(job: &BuildJob, now: DateTime<Utc>) -> BuildJob {
    let mut next = entered(job, JobState::Published, now);
    next.lease_ends_at = None;
    next
}

pub(super) fn publish_failed(job: &BuildJob, reason: &str, now: DateTime<Utc>) -> BuildJob {
    let mut next = entered(job, JobState::Failed, now);
    next.lease_ends_at = None;
    next.failure_reason = Some(log_lines::reason(Some(reason)));
    next
}
