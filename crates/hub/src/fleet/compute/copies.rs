//! The copies of the model variants on the machines (task L1-7; K11 Sections 7 and 11;
//! PULL-001, PULL-007, PULL-009, PULL-011, CMP-006). A copy is the record of the hub; its
//! state comes from the jobs of its pull and its removal, which the daemon of its machine runs
//! (K10). The hub never selects the variant or the machine here: the caller names both
//! (PULL-002 waits for the runtime choice of task L1-8).

pub mod found;
pub mod graphql;
mod model;
mod order;
mod rules;
mod store;
#[cfg(test)]
mod tests;

use chrono::{DateTime, Utc};
use daemon_core::wire::StopReason;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

pub use model::{Copy, CopyState, Origin, Status};
pub use order::actor_of;

use super::library::{self, PullSource};
use crate::{
    ids::{CopyId, DeviceId, JobId},
    work::jobs::{self, Asker, Job},
};

/// Why an operation on the copies failed.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no such copy")]
    NotFound,
    /// The last model report of the machine lists no found model with that key.
    #[error("no such found model")]
    FoundUnknown,
    #[error("the library refused the variant: {0}")]
    Library(#[from] library::Error),
    /// The files of the variant are of a kind that a machine refuses; the text says why.
    #[error("the variant cannot be pulled: {0}")]
    Unpullable(&'static str),
    /// The machine has the copy, pulls it or removes it.
    #[error("the machine already has the copy")]
    Taken(CopyState),
    #[error("the work route refused the job: {0}")]
    Jobs(#[from] jobs::Error),
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("a stored copy cannot be read")]
    Unreadable,
}

/// A copy with its state now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub copy: Copy,
    pub status: Status,
}

async fn jobs_of(
    conn: &mut PgConnection,
    copy: &Copy,
) -> Result<(Option<Job>, Option<Job>), Error> {
    let ids: Vec<_> = std::iter::once(copy.pull_job)
        .chain(copy.delete_job)
        .collect();
    let found = jobs::jobs_of(conn, &ids).await?;
    let pick = |id| found.iter().find(|job| Some(job.id) == id).cloned();
    Ok((pick(Some(copy.pull_job)), pick(copy.delete_job)))
}

/// Each copy that a machine has, pulls or removes, in the order of the machine, the model and
/// the tag. A copy whose removal is done is left out.
///
/// # Errors
/// `Database` or `Unreadable` when a record cannot be read.
pub async fn list(db: &PgPool) -> Result<Vec<Listed>, Error> {
    let mut conn = db.acquire().await?;
    let copies = store::all(&mut *conn).await?;
    let ids: Vec<_> = copies
        .iter()
        .flat_map(|copy| std::iter::once(copy.pull_job).chain(copy.delete_job))
        .collect();
    let found = jobs::jobs_of(&mut conn, &ids).await?;
    let pick = |id| found.iter().find(|job| Some(job.id) == id);
    Ok(copies
        .into_iter()
        .filter_map(|copy| {
            let status = rules::status(pick(Some(copy.pull_job)), pick(copy.delete_job))?;
            Some(Listed { copy, status })
        })
        .collect())
}

/// The variant `model:tag` that a pull takes, checked by the rules of a machine (PULL-001).
///
/// # Errors
/// `Library` for an unknown model or a missing tag; `Unpullable` for files that a machine
/// refuses.
pub async fn source_of(db: &PgPool, address: &str) -> Result<PullSource, Error> {
    let source = library::pull_source(db, address).await?;
    match order::unpullable(&source) {
        Some(why) => Err(Error::Unpullable(why)),
        None => Ok(source),
    }
}

/// Asks `device` for the variant at `now`, in the caller's transaction: a new copy, or the
/// pull of a copy that failed again with the same copy (K11 Section 11). The job writes its
/// own audit entry.
///
/// # Errors
/// `Taken` when the machine has, pulls or removes the copy; `Jobs` when the machine can never
/// run it; `Database` when it cannot be written.
pub async fn pull(
    conn: &mut PgConnection,
    (device, source): (DeviceId, &PullSource),
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<Copy, Error> {
    let actor = actor_of(asker.0.actor.kind());
    let key = (source.model.as_str(), source.variant.tag.as_str());
    let existing = store::at_locked(conn, device, key).await?;
    let again = again_or_free(conn, existing).await?;
    let wanted = |id| order::pull(id, source, (device, actor));
    let fresh = |id, job| record(id, (device, source), job, now);
    ask(conn, (again, wanted, fresh), (asker, now)).await
}

/// The copy to ask for again with its id, or `None` when the place is free: a copy whose
/// removal is done is forgotten (K11 Section 11).
///
/// # Errors
/// `Taken` when the machine has, pulls or removes the copy.
async fn again_or_free(
    conn: &mut PgConnection,
    existing: Option<Copy>,
) -> Result<Option<Copy>, Error> {
    let Some(copy) = existing else {
        return Ok(None);
    };
    let (pull, delete) = jobs_of(conn, &copy).await?;
    match rules::repull(pull.as_ref(), delete.as_ref()) {
        rules::Repull::Taken(state) => Err(Error::Taken(state)),
        rules::Repull::Replace => {
            store::forget(conn, copy.id).await?;
            Ok(None)
        }
        rules::Repull::Again => Ok(Some(copy)),
    }
}

/// Queues the job that brings the copy: for the copy `again` with its id, else for a new copy
/// that `fresh` makes, which is written.
async fn ask(
    conn: &mut PgConnection,
    (again, wanted, fresh): (
        Option<Copy>,
        impl FnOnce(CopyId) -> jobs::NewJob,
        impl FnOnce(CopyId, JobId) -> Copy,
    ),
    (asker, now): (Asker, DateTime<Utc>),
) -> Result<Copy, Error> {
    let id = again
        .as_ref()
        .map_or_else(|| CopyId::from(Uuid::now_v7()), |copy| copy.id);
    let job = jobs::enqueue_in(conn, wanted(id), asker, now).await?;
    if let Some(copy) = again {
        store::set_jobs(conn, copy.id, (job.id, None)).await?;
        return Ok(Copy {
            pull_job: job.id,
            delete_job: None,
            ..copy
        });
    }
    let copy = fresh(id, job.id);
    store::insert(conn, &copy).await?;
    Ok(copy)
}

fn record(
    id: CopyId,
    (device, source): (DeviceId, &PullSource),
    job: JobId,
    now: DateTime<Utc>,
) -> Copy {
    let variant = &source.variant;
    Copy {
        id,
        device,
        variant: Some(source.variant_id),
        model: source.model.clone(),
        tag: variant.tag.clone(),
        format: variant.format,
        quantisation: variant.quantisation.clone(),
        repository: variant.repository.clone(),
        revision: variant.revision.clone(),
        size_bytes: variant.size_bytes(),
        files: u32::try_from(variant.files.len()).unwrap_or(u32::MAX),
        pull_job: job,
        delete_job: None,
        created_at: now,
        origin: Origin::Pulled,
        found: None,
    }
}

/// What a removal left: the copy as it was, and the record that stays until its
/// `copy_delete` is done; `None` when the record went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removed {
    pub copy: Copy,
    pub kept: Option<Copy>,
}

/// Removes the copy at `now`, in the caller's transaction (PULL-009): a pull that waits in the
/// queue stops and the record goes; a pull that a machine holds stops; a copy that a machine
/// may hold gets a `copy_delete`, which the machine runs after the pull ends.
///
/// # Errors
/// `NotFound` for no such copy; `Jobs` or `Database` when the change cannot be written.
pub async fn remove(
    conn: &mut PgConnection,
    id: CopyId,
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<Removed, Error> {
    let copy = store::locked(conn, id).await?.ok_or(Error::NotFound)?;
    let (pull, delete) = jobs_of(conn, &copy).await?;
    let pull = pull.ok_or(Error::Unreadable)?;
    let stop = (pull.id, StopReason::Operator);
    let next = match rules::removal(&pull, delete.as_ref()) {
        rules::Removal::StopQueued => {
            let stopped = jobs::stop_in(conn, stop, asker.clone(), now).await?;
            if stopped.is_some_and(|job| job.state == jobs::State::Stopped) {
                rules::Removal::Forget
            } else {
                rules::Removal::Delete
            }
        }
        rules::Removal::StopAndDelete => {
            jobs::stop_in(conn, stop, asker.clone(), now).await?;
            rules::Removal::Delete
        }
        other => other,
    };
    match next {
        rules::Removal::Forget => {
            store::forget(conn, id).await?;
            Ok(Removed { copy, kept: None })
        }
        rules::Removal::Delete => {
            let actor = actor_of(asker.0.actor.kind());
            let wanted = order::delete(id, (copy.device, actor));
            let job = jobs::enqueue_in(conn, wanted, asker, now).await?;
            store::set_jobs(conn, id, (copy.pull_job, Some(job.id))).await?;
            let kept = Copy {
                delete_job: Some(job.id),
                ..copy.clone()
            };
            Ok(Removed {
                copy,
                kept: Some(kept),
            })
        }
        _ => Ok(Removed {
            kept: Some(copy.clone()),
            copy,
        }),
    }
}

/// The copy with its state now; `None` when its removal is done.
///
/// # Errors
/// `Database` or `Unreadable` when a record cannot be read.
pub async fn listed(conn: &mut PgConnection, copy: Copy) -> Result<Option<Listed>, Error> {
    let (pull, delete) = jobs_of(conn, &copy).await?;
    Ok(rules::status(pull.as_ref(), delete.as_ref()).map(|status| Listed { copy, status }))
}
