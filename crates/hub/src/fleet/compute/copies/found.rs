//! The found models of the machines and their use where they are (task L1-22; K11 Sections
//! 2.3 and 8; PULL-012, R-444). The hub reads the found models from the last `model_report`
//! of each machine, kept in the result of that job, and keeps no second list. A found model
//! that a copy uses is not listed.

use std::collections::HashSet;

use chrono::{DateTime, Utc};
use daemon_core::wire::{FoundEntry, WorkType};
use sqlx::{PgConnection, PgPool};

use super::super::library::Format;
use super::{Copy, Error, Origin, actor_of, again_or_free, ask, order, store};
use crate::{
    ids::{CopyId, DeviceId, JobId},
    work::jobs::{self, Asker, Job},
};

/// A model that a machine found, as its last model report listed it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub device: DeviceId,
    pub entry: FoundEntry,
    /// When the machine ended the report that listed it.
    pub read_at: DateTime<Utc>,
}

fn listed_by(job: &Job) -> Vec<Found> {
    let (Some(device), Some(read_at)) = (job.asked.machine, job.ended_at) else {
        return Vec::new();
    };
    let entries = job.result.as_ref().and_then(|result| result.found());
    entries
        .unwrap_or_default()
        .iter()
        .map(|entry| Found {
            device,
            entry: entry.clone(),
            read_at,
        })
        .collect()
}

/// The found models of the last report of each machine.
async fn reported(conn: &mut PgConnection) -> Result<Vec<Found>, Error> {
    let reports = jobs::newest_done(conn, WorkType::ModelReport).await?;
    Ok(reports.iter().flat_map(listed_by).collect())
}

/// Each found model that no copy uses, in the order of the machine and the name.
///
/// # Errors
/// `Database`, `Jobs` or `Unreadable` when a record cannot be read.
pub async fn list(db: &PgPool) -> Result<Vec<Found>, Error> {
    let used: HashSet<(DeviceId, String)> = super::list(db)
        .await?
        .into_iter()
        .filter_map(|listed| Some((listed.copy.device, listed.copy.found?)))
        .collect();
    let mut conn = db.acquire().await?;
    let mut found: Vec<Found> = reported(&mut conn)
        .await?
        .into_iter()
        .filter(|found| !used.contains(&(found.device, found.entry.found.clone())))
        .collect();
    found.sort_by(|a, b| {
        (a.device.to_string(), &a.entry.name).cmp(&(b.device.to_string(), &b.entry.name))
    });
    Ok(found)
}

/// Asks `device` for its model state at `now`, in the caller's transaction, so that its found
/// models are read again (K11 Section 8). The job writes its own audit entry.
///
/// # Errors
/// `Jobs` when the machine can never run it; `Database` when it cannot be written.
pub async fn read(
    conn: &mut PgConnection,
    device: DeviceId,
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<JobId, Error> {
    let actor = actor_of(asker.0.actor.kind());
    let job = jobs::enqueue_in(conn, order::report((device, actor)), asker, now).await?;
    Ok(job.id)
}

/// Uses the found model with the key `found` of `device` where it is, at `now`, in the
/// caller's transaction: a new found copy and its `copy_adopt`, or the `copy_adopt` of a found
/// copy that failed again (K11 Section 2.3).
///
/// # Errors
/// `FoundUnknown` when the last report of the machine has no such model; `Unpullable` for a
/// model that is not GGUF; `Taken` when a copy uses it; `Jobs` or `Database` as for a pull.
pub async fn adopt(
    conn: &mut PgConnection,
    (device, found): (DeviceId, &str),
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<Copy, Error> {
    let entry = reported(conn)
        .await?
        .into_iter()
        .find(|listed| listed.device == device && listed.entry.found == found)
        .ok_or(Error::FoundUnknown)?
        .entry;
    if !entry.usable {
        return Err(Error::Unpullable(
            "Only a GGUF model can be used where it is.",
        ));
    }
    let actor = actor_of(asker.0.actor.kind());
    let existing = store::found_locked(conn, device, found).await?;
    let again = again_or_free(conn, existing).await?;
    let wanted = |id| order::adopt(id, found, (device, actor));
    let fresh = |id, job| record(id, (device, &entry), job, now);
    ask(conn, (again, wanted, fresh), (asker, now)).await
}

fn record(
    id: CopyId,
    (device, entry): (DeviceId, &FoundEntry),
    job: JobId,
    now: DateTime<Utc>,
) -> Copy {
    Copy {
        id,
        device,
        variant: None,
        model: entry.name.clone(),
        tag: String::new(),
        format: Format::Gguf,
        quantisation: String::new(),
        repository: entry.tool.clone(),
        revision: String::new(),
        size_bytes: entry.bytes,
        files: entry.files,
        pull_job: job,
        delete_job: None,
        created_at: now,
        origin: Origin::Found,
        found: Some(entry.found.clone()),
    }
}
