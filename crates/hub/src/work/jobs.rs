//! The job queue of the work route (K10, R-209; PRO-050, PRO-051).
//! The hub never names a command: a job has a type of a fixed list and that type's object.

mod application_apply;
mod artifacts;
mod claiming;
mod enqueue;
mod hosting;
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
use daemon_core::wire::{HostItem, Reported, StopReason, WorkJob, WorkType};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use enqueue::queued;

pub use application_apply::{
    ApplicationFiles, ReleasedApplicationFile, enqueue_application_apply, take_application_file,
};
pub use artifacts::{ArtifactAccess, ArtifactGuard, artifact_access};
pub(crate) use checkout::{CheckoutSource, SourceSubmission};
pub(super) use claiming::checkout;
pub use claiming::claim;
pub use enqueue::enqueue;
pub(crate) use enqueue::enqueue_source;
pub use hosting::enqueue_container_action;
pub use model::{Fault, Job, Machine, NewJob, State};
pub(crate) use pinning::{SourceBuildPlacement, SourceNode, set_source_pin, source_placement};
pub use pinning::{exclude, pin_project, placement_reason, unpin_project};
pub use placement::{Unfit, Verdict};
pub use reasons::Decision;
pub use reporting::{end_lost, report};

use crate::{
    fleet::events::{self, Bus, ClientAddr, Operator},
    ids::JobId,
};

/// Read-only projection of a source build, scoped to the source recipe.
///
/// # Errors
/// Database failure or an unreadable job.
pub async fn source_job(db: &PgPool, id: JobId) -> Result<Option<Job>, Error> {
    store::source_job(db, id).await
}

/// Read an action result without exposing any other work kind.
///
/// # Errors
/// Database or stored-record failure.
pub async fn container_action(db: &PgPool, id: JobId) -> Result<Option<Job>, Error> {
    store::hosting::request(db, id, WorkType::ContainerAction).await
}

/// Read the metadata of a managed Compose apply by its job ID.
///
/// # Errors
/// Database or stored-record failure.
pub async fn application_apply(db: &PgPool, id: JobId) -> Result<Option<Job>, Error> {
    store::hosting::request(db, id, WorkType::ApplicationApply).await
}

/// Completed managed applications of this device among the observed IDs.
///
/// # Errors
/// Database failure.
pub(crate) async fn installed_applications(
    db: &PgPool,
    device: crate::ids::DeviceId,
    candidates: &[Uuid],
) -> Result<std::collections::HashSet<Uuid>, Error> {
    store::hosting::installed_applications(db, device, candidates).await
}

/// Devices with a recent paired Compose apply advertisement.
///
/// # Errors
/// Database failure.
pub async fn application_apply_targets(
    db: &PgPool,
    now: DateTime<Utc>,
) -> Result<Vec<(crate::ids::DeviceId, String)>, Error> {
    store::hosting::apply_targets(db, now - placement::FRESH).await
}

/// Whether the named daemon recently advertised its paired hosting service.
///
/// # Errors
/// Database failure.
pub async fn container_actions_available(
    db: &PgPool,
    device: crate::ids::DeviceId,
    now: DateTime<Utc>,
) -> Result<bool, Error> {
    store::hosting::available(db, device, now - placement::FRESH).await
}

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
    Host(Box<HostItem>),
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
