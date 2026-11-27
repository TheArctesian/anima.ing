//! Pins, exclusions and the reason of a placement (ROU-004, ROU-006; R-259): work that names
//! one machine goes to no other, and an excluded machine gets only work that names it.

mod source;

use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgConnection;

use super::{Asker, Error, Queue, ended, model::Job, placement, reasons, store, trail};
use crate::ids::{DeviceId, JobId, ProjectId};
pub(crate) use source::{SourceBuildPlacement, SourceNode, set_source_pin, source_placement};

/// A machine that only has too little free memory or disk now is not refused: the job waits.
pub(super) async fn named_can_run(
    conn: &mut PgConnection,
    job: &Job,
    now: DateTime<Utc>,
) -> Result<(), Error> {
    let pins = store::pins(conn).await?;
    let Some(device) = placement::named(job, &pins) else {
        if placement::needs_private_checkout(&job.asked) {
            return Err(Error::Invalid(
                "Pin this project to an AnimaOS node before queueing a private build.",
            ));
        }
        return Ok(());
    };
    let machine = store::machine_locked(conn, device).await?;
    let offer = store::offer_of(conn, device).await?;
    if placement::needs_private_checkout(&job.asked)
        && !offer
            .as_ref()
            .is_some_and(|offer| placement::recipe_fits(job, offer))
    {
        return Err(Error::Invalid(
            "The pinned node does not support private source checkout.",
        ));
    }
    let known = machine.as_ref().zip(offer.as_ref());
    match placement::never_fits(job, known, now) {
        Some(why) => Err(Error::NoFit(reasons::unfit(job, why))),
        None => Ok(()),
    }
}

/// False when there is no such device.
///
/// # Errors
/// `Database` or `Events` when the change cannot be written; nothing is changed then.
pub async fn pin_project(
    queue: Queue<'_>,
    (project, device): (ProjectId, DeviceId),
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<bool, Error> {
    let mut tx = queue.db.begin().await?;
    let outcome = pin_in(&mut tx, (project, device), asker, now).await;
    ended(tx, outcome).await
}

async fn pin_in(
    conn: &mut PgConnection,
    (project, device): (ProjectId, DeviceId),
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<bool, Error> {
    let Some(changed) = store::pin(conn, project, device).await? else {
        return Ok(false);
    };
    if changed {
        let change = json!({ "pinnedForProject": project });
        trail::placement_set(conn, (device, change), (asker, now)).await?;
    }
    Ok(true)
}

/// False when the project had no pin.
///
/// # Errors
/// `Database` or `Events` when the change cannot be written; nothing is changed then.
pub async fn unpin_project(
    queue: Queue<'_>,
    project: ProjectId,
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<bool, Error> {
    let mut tx = queue.db.begin().await?;
    let outcome = unpin_in(&mut tx, project, asker, now).await;
    ended(tx, outcome).await
}

async fn unpin_in(
    conn: &mut PgConnection,
    project: ProjectId,
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<bool, Error> {
    let Some(device) = store::unpin(conn, project).await? else {
        return Ok(false);
    };
    let change = json!({ "unpinnedForProject": project });
    trail::placement_set(conn, (device, change), (asker, now)).await?;
    Ok(true)
}

/// False for a machine that never asked this hub for work.
///
/// # Errors
/// `Database` or `Events` when the change cannot be written; nothing is changed then.
pub async fn exclude(
    queue: Queue<'_>,
    (device, excluded): (DeviceId, bool),
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<bool, Error> {
    let mut tx = queue.db.begin().await?;
    let outcome = async {
        if !store::set_excluded(&mut tx, device, excluded).await? {
            return Ok(false);
        }
        let change = json!({ "excluded": excluded });
        trail::placement_set(&mut tx, (device, change), (asker, now)).await?;
        Ok(true)
    }
    .await;
    ended(tx, outcome).await
}

/// The line is data (MCP-008).
///
/// # Errors
/// `Database` when it cannot be read.
pub async fn placement_reason(queue: Queue<'_>, id: JobId) -> Result<Option<String>, Error> {
    let mut tx = queue.db.begin().await?;
    let outcome = async {
        let Some(job) = store::job_locked(&mut tx, id).await? else {
            return Ok(None);
        };
        let decisions = store::decisions_of(&mut tx, id).await?;
        Ok(Some(reasons::line(&job, &decisions)))
    }
    .await;
    ended(tx, outcome).await
}
