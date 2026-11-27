//! Identity of a private repository build selected by the signed-in operator.

use std::num::NonZeroU64;

use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::wire::{Switch, WorkKind};
use sqlx::PgConnection;
use uuid::Uuid;

use super::super::{Error, Job, State, placement::needs_private_checkout, store};
use crate::{
    clock::Clock,
    ids::{DeviceId, JobId, RepositoryId, SourceAccountId},
};

pub(in crate::work) use super::super::trail::secret_released;
pub(in crate::work) use store::checkout::{Revocation, due, reserve, revoked, save};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct CheckoutSource {
    pub account: SourceAccountId,
    pub forge_repository: NonZeroU64,
}

pub(crate) struct SourceSubmission {
    pub request: Uuid,
    pub repository: RepositoryId,
    pub checkout: Option<CheckoutSource>,
}

pub(in crate::work) struct Held {
    pub job: Job,
    pub repository: RepositoryId,
    pub source: CheckoutSource,
    pub deadline: DateTime<Utc>,
}

pub(in crate::work) async fn held(
    conn: &mut PgConnection,
    device: DeviceId,
    (job, attempt): (JobId, u32),
    clock: &dyn Clock,
) -> Result<Option<Held>, Error> {
    let Some(machine) = store::machine_locked(conn, device).await? else {
        return Ok(None);
    };
    let Some(job) = store::job_locked(conn, job).await? else {
        return Ok(None);
    };
    let now = clock.now();
    if machine.switch != Switch::On
        || machine.unclean
        || machine.held_off(now)
        || job.holder != Some(device)
        || job.attempt != attempt
        || job.stop.is_some()
        || !matches!(job.state, State::Preparing | State::Running)
        || !needs_private_checkout(&job.asked)
        || job.asked.kind != WorkKind::Container
    {
        return Ok(None);
    }
    let Some(deadline) = expiry(&job).filter(|&expires| now < expires) else {
        return Ok(None);
    };
    let Some(project) = job.asked.project else {
        return Ok(None);
    };
    if store::checkout::locked_pin(conn, project).await? != Some(device) {
        return Ok(None);
    }
    let Some((repository, source)) = store::checkout::source(conn, job.id).await? else {
        return Ok(None);
    };
    Ok(Some(Held {
        job,
        repository,
        source,
        deadline,
    }))
}

fn expiry(job: &Job) -> Option<DateTime<Utc>> {
    let seconds = i64::try_from(job.asked.limits.seconds).ok()?;
    let limit = job
        .claimed_at?
        .checked_add_signed(TimeDelta::try_seconds(seconds)?)?;
    Some(job.lease_ends_at?.min(limit))
}

pub(in crate::work) fn seal_context(
    job: JobId,
    attempt: impl std::fmt::Display,
    device: DeviceId,
) -> String {
    format!("jobs.repository_token_releases:{job}:{attempt}:{device}")
}
