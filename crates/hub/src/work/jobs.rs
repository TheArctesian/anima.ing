//! The job queue of the work route (K10, R-209; PRO-050, PRO-051).
//! The hub never names a command: a job has a type of a fixed list and that type's object.

mod claiming;
mod lifecycle;
mod model;
mod pinning;
mod placement;
mod reasons;
mod reporting;
mod results;
mod store;
#[cfg(test)]
mod tests;
mod trail;
mod work_switch;

use chrono::{DateTime, Utc};
use daemon_core::wire::{Reported, StopReason, WorkJob, WorkType};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub use claiming::claim;
pub use model::{Fault, Job, Machine, NewJob, State};
pub use pinning::{exclude, pin_project, placement_reason, unpin_project};
pub use placement::{Unfit, Verdict};
pub use reasons::Decision;
pub use reporting::{end_lost, report};
pub use work_switch::{blocks_dispatch, set_device_work};

use crate::{
    fleet::events::{self, Bus, ClientAddr, Operator},
    ids::JobId,
};

pub const LEASE_SWEEP: std::time::Duration = std::time::Duration::from_secs(30);

/// A device that never asked for work has no entry (USE-004, CMP-002).
///
/// # Errors
/// `Database` when the query fails; `Unreadable` for a stored row this version does not read.
pub async fn machines(
    db: &PgPool,
) -> Result<std::collections::HashMap<crate::ids::DeviceId, Machine>, Error> {
    store::machines(db).await
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the job is not valid: {0}")]
    Invalid(&'static str),
    /// No different machine is taken in its place (ROU-006, JOB-014).
    #[error("the job names a machine that cannot run it: {0}")]
    NoFit(String),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("a stored job row cannot be read")]
    Unreadable,
    #[error("the event log cannot be written")]
    Events(#[from] events::Error),
}

#[derive(Clone, Copy)]
pub struct Queue<'a> {
    pub db: &'a PgPool,
    pub bus: &'a Bus,
}

pub type Asker = (Operator, ClientAddr);

#[derive(Debug, PartialEq)]
pub enum Claimed {
    /// No job for this machine now (`204`).
    Nothing,
    /// The hub gives this machine no work (`403 WORK_OFF`).
    Off,
    Job(Box<WorkJob>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Reporting {
    Reported(Reported),
    /// `409 NOT_HOLDER`.
    NotHolder,
    /// `400 MALFORMED`.
    Malformed,
    /// `403 WORK_OFF`.
    Off,
}

pub(super) async fn ended<T>(
    tx: Transaction<'_, Postgres>,
    outcome: Result<T, Error>,
) -> Result<T, Error> {
    match outcome {
        Ok(value) => {
            tx.commit().await?;
            Ok(value)
        }
        Err(error) => {
            tx.rollback().await?;
            Err(error)
        }
    }
}

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
    use daemon_core::wire::WorkKind;
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
    validate(&wanted)?;
    let job = Job {
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
    };
    let mut tx = queue.db.begin().await?;
    let outcome = async {
        pinning::named_can_run(&mut tx, &job, now).await?;
        store::insert(&mut tx, &job).await?;
        trail::queued(&mut tx, &job, asker, now).await
    }
    .await;
    ended(tx, outcome).await.map(|()| job)
}

/// A queued job stops at once; a held job's holder gets it in its next report (K10 Section 7).
///
/// # Errors
/// `Database` or `Events` when the change cannot be written; nothing is changed then.
pub async fn stop(
    queue: Queue<'_>,
    (id, reason): (JobId, StopReason),
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<Option<Job>, Error> {
    let mut tx = queue.db.begin().await?;
    let outcome = async {
        let Some(job) = store::job_locked(&mut tx, id).await? else {
            return Ok(None);
        };
        let Some(next) = lifecycle::stop_asked(&job, reason, now) else {
            return Ok(None);
        };
        store::save(&mut tx, &next).await?;
        trail::stop_asked(&mut tx, &next, reason, (asker, now)).await?;
        Ok(Some(next))
    }
    .await;
    ended(tx, outcome).await
}

/// # Errors
/// `Database` when it cannot be read.
pub async fn job_with_log(queue: Queue<'_>, id: JobId) -> Result<Option<(Job, String)>, Error> {
    let mut tx = queue.db.begin().await?;
    let outcome = async {
        let Some(job) = store::job_locked(&mut tx, id).await? else {
            return Ok(None);
        };
        let log = store::log_of(&mut tx, id).await?;
        Ok(Some((job, log)))
    }
    .await;
    ended(tx, outcome).await
}
