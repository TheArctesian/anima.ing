//! Job admission and insertion, shared by ordinary and replayable submissions.
mod source_submission;

pub(crate) use source_submission::enqueue_source;

use super::{Asker, Error, Job, NewJob, Queue, State, ended, hosting, pinning, store, trail};
use crate::ids::JobId;
use chrono::{DateTime, Utc};
use daemon_core::wire::{WorkKind, WorkType};
use uuid::Uuid;

/// The secret purposes each job type may name (K10 Sections 6 and 8).
fn secrets_permitted(job: &NewJob) -> bool {
    let permitted: &[&str] = match job.work_type {
        WorkType::Runner => &["jit_config"],
        WorkType::Build => &["repository_token", "project_secret"],
        _ => &[],
    };
    job.secrets
        .iter()
        .all(|purpose| permitted.contains(&purpose.as_str()))
}

fn validate(job: &NewJob) -> Result<(), Error> {
    if job.work_type == WorkType::Unknown || job.kind == WorkKind::Unknown {
        return Err(Error::Invalid(
            "the type and the kind are of the fixed lists",
        ));
    }
    if job.work_type.is_model() || job.kind == WorkKind::Model {
        return Err(Error::Invalid("the hub does not make model items yet"));
    }
    if !job.parameters.is_object() {
        return Err(Error::Invalid("the parameters are an object"));
    }
    if !secrets_permitted(job) {
        return Err(Error::Invalid(
            "a secret purpose is not one of the type of the job",
        ));
    }
    let limits = job.limits;
    let all = [
        limits.seconds,
        limits.idle_seconds,
        limits.memory_bytes,
        limits.disk_bytes,
        limits.orphan_seconds,
    ];
    if all
        .iter()
        .any(|&limit| limit == 0 || i64::try_from(limit).is_err())
    {
        return Err(Error::Invalid("each limit is above 0"));
    }
    if job.kind == WorkKind::Host {
        return hosting::validate(job);
    }
    if matches!(
        job.work_type,
        WorkType::ContainerAction | WorkType::ApplicationApply
    ) || job.repository.is_none()
    {
        return Err(Error::Invalid(
            "a job must have a repository and use job isolation",
        ));
    }
    Ok(())
}

/// Puts a job in the queue at `now`, with its audit entry (K10 Section 10).
///
/// # Errors
/// `Invalid`, `NoFit` for a named or pinned machine that can never run it, `Database`, `Events`.
pub async fn enqueue(
    queue: Queue<'_>,
    wanted: NewJob,
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<Job, Error> {
    let wanted = wanted.bind_source_target()?;
    if super::placement::needs_private_checkout(&wanted) {
        return Err(Error::Invalid(
            "private source builds require a connected repository binding",
        ));
    }
    validate(&wanted)?;
    let job = queued(wanted, now);
    let mut tx = queue.db.begin().await?;
    let outcome = insert_queued(&mut tx, &job, asker, now).await;
    ended(tx, outcome).await.map(|()| job)
}

async fn insert_queued(
    tx: &mut sqlx::PgConnection,
    job: &Job,
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<(), Error> {
    let key = if job.asked.kind == WorkKind::Host {
        hosting::admit(tx, job, &asker.0, now).await?
    } else {
        pinning::named_can_run(tx, job, now).await?;
        None
    };
    store::insert(tx, job, key).await?;
    trail::queued(tx, job, asker, now).await
}

pub(super) fn queued(wanted: NewJob, now: DateTime<Utc>) -> Job {
    Job {
        id: JobId::from(Uuid::now_v7()),
        asked: wanted,
        state: State::Queued,
        attempt: 0,
        lost: 0,
        holder: None,
        last_device: None,
        refused_by: Vec::new(),
        lease_ends_at: None,
        claimed_at: None,
        stop: None,
        reason: None,
        result: None,
        log_end: 0,
        created_at: now,
        changed_at: now,
        ended_at: None,
    }
}
