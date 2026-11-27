//! Job admission and insertion, shared by ordinary and replayable submissions.
mod source_submission;

pub(crate) use source_submission::enqueue_source;

use super::{
    Asker, Error, Job, NewJob, Queue, State, Target, ended, hosting, pinning, store, trail,
};
use crate::ids::JobId;
use chrono::{DateTime, Utc};
use daemon_core::wire::{WorkKind, WorkType};
use sqlx::PgConnection;
use uuid::Uuid;

/// The secret purposes each job type may name (K10 Sections 6 and 8).
fn secrets_permitted(job: &NewJob) -> bool {
    let permitted: &[&str] = match job.work_type {
        WorkType::Runner => &["jit_config"],
        WorkType::Build => &["repository_token", "project_secret"],
        // The token of a gated repository (K11 Section 7, PULL-010).
        WorkType::ModelPull => &["hf_token"],
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
    let model_target = matches!(job.target, Target::Model { .. });
    if job.work_type.is_model() != (job.kind == WorkKind::Model)
        || model_target != (job.kind == WorkKind::Model)
    {
        return Err(Error::Invalid(
            "a model item has a model type, the kind model and an actor",
        ));
    }
    if model_target && job.project.is_some() {
        return Err(Error::Invalid("a model item has no project"));
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
    if job.kind == WorkKind::Model {
        return Ok(());
    }
    if job.kind == WorkKind::Host {
        return hosting::validate(job);
    }
    if matches!(
        job.work_type,
        WorkType::ContainerAction | WorkType::ApplicationApply
    ) || job.target.repository().is_none()
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
    let mut tx = queue.db.begin().await?;
    let outcome = enqueue_in(&mut tx, wanted, asker, now).await;
    ended(tx, outcome).await
}

/// [`enqueue`] in the caller's transaction, so that the record of the function that makes the
/// work and its job are written together (the copies of the compute domain).
///
/// # Errors
/// As [`enqueue`].
pub async fn enqueue_in(
    conn: &mut PgConnection,
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
    insert_queued(conn, &job, asker, now).await?;
    Ok(job)
}

async fn insert_queued(
    tx: &mut PgConnection,
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
        progress: None,
        created_at: now,
        changed_at: now,
        ended_at: None,
    }
}
