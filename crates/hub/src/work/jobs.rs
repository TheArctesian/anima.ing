//! The jobs of the work route (K10, `contracts/work-route.md`, accepted by R-209; PRO-050,
//! PRO-051; task W3): the queue from which the hub gives work to the daemon of a machine.
//!
//! A job is what the contract calls an item of work. The hub never names a command: a job has a
//! type of a fixed list and the object of that type. The rules are pure (`lifecycle`,
//! `placement`); this file runs them in one transaction with their audit entry (K10 Section 10).
//! The HTTP routes are the daemon's edge in `fleet::metrics::ingest`.

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

use chrono::{DateTime, Utc};
use daemon_core::wire::{Reported, StopReason, WorkType};
use futures_util::future::BoxFuture;
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

pub use claiming::claim;
pub use claiming::{Claimant, ModelWork, Work};
pub use model::{Fault, Job, Machine, NewJob, State, Target};
pub use pinning::{exclude, pin_project, placement_reason, unpin_project};
pub use placement::{Unfit, Verdict};
pub use reasons::Decision;
pub use reporting::{end_lost, report};

use crate::{
    fleet::events::{self, Bus, ClientAddr, Operator},
    ids::JobId,
};

/// How often the upkeep ends the leases that ran out and the attempts that are too long.
pub const LEASE_SWEEP: std::time::Duration = std::time::Duration::from_secs(30);

/// What the hub knows of each machine for work (the switch "takes work", USE-004); a device
/// that never asked for work has no entry. The device policy of compute reads it (CMP-002).
///
/// # Errors
/// `Database` when the query fails; `Unreadable` for a stored row this version does not read.
pub async fn machines(
    db: &PgPool,
) -> Result<std::collections::HashMap<crate::ids::DeviceId, Machine>, Error> {
    store::machines(db).await
}

/// Why an operation on the jobs failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The job that a caller wants in the queue breaks a rule of K10; the text names the rule.
    #[error("the job is not valid: {0}")]
    Invalid(&'static str),
    /// The machine that the job names, or that is pinned for its project, can never run it;
    /// the text is the reason. No different machine is taken in its place (ROU-006, JOB-014).
    #[error("the job names a machine that cannot run it: {0}")]
    NoFit(String),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    /// A stored row is not in a form this version reads.
    #[error("a stored job row cannot be read")]
    Unreadable,
    #[error("the event log cannot be written")]
    Events(#[from] events::Error),
    /// The device policy of the machine that asks cannot be read; the wiring logs its cause.
    #[error("the device policy cannot be read")]
    Policy,
}

/// Reads whether the device policy of a machine permits model work at a time (CAP-010 to
/// CAP-012). The wiring gives it, so neither this domain nor the work route imports the policy.
pub type PolicyGate = std::sync::Arc<
    dyn Fn(crate::ids::DeviceId, DateTime<Utc>) -> BoxFuture<'static, Result<bool, Error>>
        + Send
        + Sync,
>;

/// What the jobs need to work: the database and the bus of the events.
#[derive(Clone, Copy)]
pub struct Queue<'a> {
    pub db: &'a PgPool,
    pub bus: &'a Bus,
}

/// Who asks, for the audit entry: the operator (or the hub) and the address of the request.
pub type Asker = (Operator, ClientAddr);

/// The answer to a claim.
#[derive(Debug, PartialEq)]
pub enum Claimed {
    /// No job for this machine now (`204`).
    Nothing,
    /// The hub gives this machine no work (`403 WORK_OFF`).
    Off,
    Job(Box<Work>),
}

/// The answer to a report.
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

/// The purposes that a job of each type can name (K10 Sections 6 and 8).
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
    use daemon_core::wire::WorkKind;
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
    Ok(())
}

/// Puts a job in the queue at `now`, with its audit entry (K10 Section 10). The callers are the
/// functions of the hub that make work: the runners of a project and the builds.
///
/// # Errors
/// [`Error::Invalid`] for a job that breaks a rule of K10; [`Error::NoFit`] for a job whose
/// named or pinned machine can never run it; `Database` or `Events` when it cannot be written.
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
        progress: None,
        created_at: now,
        changed_at: now,
        ended_at: None,
    };
    pinning::named_can_run(conn, &job, now).await?;
    store::insert(conn, &job).await?;
    trail::queued(conn, &job, asker, now).await?;
    Ok(job)
}

/// The jobs with these ids; an id with no job is left out.
///
/// # Errors
/// `Database` when they cannot be read; `Unreadable` for a row this version does not read.
pub async fn jobs_of(conn: &mut PgConnection, ids: &[JobId]) -> Result<Vec<Job>, Error> {
    store::jobs_of(conn, ids).await
}

/// Asks for the stop of a job at `now` (K10 Section 7). A queued job stops at once; the holder
/// of a held job gets the stop in the answer to its next report. `None` for a job that is not
/// there or has ended.
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
    let outcome = stop_in(&mut tx, (id, reason), asker, now).await;
    ended(tx, outcome).await
}

/// [`stop`] in the caller's transaction, so that a copy and the stop of its pull are written
/// together (the copies of the compute domain).
///
/// # Errors
/// As [`stop`].
pub async fn stop_in(
    conn: &mut PgConnection,
    (id, reason): (JobId, StopReason),
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<Option<Job>, Error> {
    let Some(job) = store::job_locked(conn, id).await? else {
        return Ok(None);
    };
    let Some(next) = lifecycle::stop_asked(&job, reason, now) else {
        return Ok(None);
    };
    store::save(conn, &next).await?;
    trail::stop_asked(conn, &next, reason, (asker, now)).await?;
    Ok(Some(next))
}

/// The job and the log that the hub holds of it; `None` when there is no such job.
///
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
