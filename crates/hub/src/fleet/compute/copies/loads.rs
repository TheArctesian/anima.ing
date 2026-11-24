//! The copies in the memory of their machines, and their idle time and pin (task L1-16; K11 Section 11.1;
//! FIT-020 to FIT-025). The hub reads the loads from the last `model_report` of each machine, as it reads the
//! found models, and keeps no second list. A pin and an idle time are a `model_keep` to the machine of the copy,
//! which keeps them; the hub sees them in the next report while the copy is loaded.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use daemon_core::model_work::memory::keep::Keep;
use daemon_core::wire::WorkType;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use super::{Copy, CopyState, Error, actor_of, order, store};
use crate::{
    ids::{CopyId, DeviceId, JobId},
    work::jobs::{self, Asker, Job},
};

pub use super::model::LoadState;

/// A copy that a server of its machine holds, as the last model report of the machine gave it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Load {
    pub server: Uuid,
    pub state: LoadState,
    pub reserved_bytes: u64,
    pub idle_seconds: u32,
    pub pinned: bool,
    /// Rounded down to a minute by the machine; 0 while the copy is `active` or `loading`.
    pub used_seconds_ago: u64,
    /// When the machine ended the report.
    pub read_at: DateTime<Utc>,
}

fn loads_of(job: &Job) -> Vec<((DeviceId, CopyId), Load)> {
    let (Some(device), Some(read_at)) = (job.asked.machine, job.ended_at) else {
        return Vec::new();
    };
    let servers = job.result.as_ref().and_then(|result| result.servers());
    servers
        .unwrap_or_default()
        .iter()
        .flat_map(|server| {
            server
                .models
                .iter()
                .map(move |entry| (server.server, entry))
        })
        .filter_map(|(server, entry)| {
            let load = Load {
                server,
                state: LoadState::from_reported(&entry.state)?,
                reserved_bytes: entry.reserved_bytes,
                idle_seconds: entry.idle_seconds,
                pinned: entry.pinned,
                used_seconds_ago: entry.used_seconds_ago,
                read_at,
            };
            Some(((device, CopyId::from(entry.copy)), load))
        })
        .collect()
}

/// The loads of the last report of each machine, by the machine and the copy: a machine can name only its own
/// copies.
///
/// # Errors
/// `Jobs` when a report cannot be read.
pub async fn by_copy(db: &PgPool) -> Result<HashMap<(DeviceId, CopyId), Load>, Error> {
    let mut conn = db.acquire().await?;
    let reports = jobs::newest_done(&mut conn, WorkType::ModelReport).await?;
    Ok(reports.iter().flat_map(loads_of).collect())
}

/// Asks the machine of the copy to keep it loaded for `keep.idle_seconds` after its last use, and to pin it or end
/// its pin, at `now`, in the caller's transaction (`model_keep`, K11 Section 11.1). The job writes its own audit
/// entry.
///
/// # Errors
/// `NotFound` for no such copy; `NotOnMachine` for a copy that its machine does not have now; `Jobs` or `Database`
/// when the job cannot be written.
pub async fn keep(
    conn: &mut PgConnection,
    (id, keep): (CopyId, Keep),
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<(Copy, JobId), Error> {
    let copy = store::locked(conn, id).await?.ok_or(Error::NotFound)?;
    let listed = super::listed(conn, copy.clone())
        .await?
        .ok_or(Error::NotFound)?;
    if !matches!(
        listed.status.state,
        CopyState::Downloaded | CopyState::Validated
    ) {
        return Err(Error::NotOnMachine(listed.status.state));
    }
    let actor = actor_of(asker.0.actor.kind());
    let wanted = order::keep(id, keep, (copy.device, actor));
    let job = jobs::enqueue_in(conn, wanted, asker, now).await?;
    Ok((copy, job.id))
}
