//! Prunes files tracked by Builds after publication (BLD-086).

use std::{cmp::Reverse, time::Duration};

use sqlx::PgPool;

use super::{
    feed,
    run::Reach,
    transfer::{self, Stop},
};
use crate::fleet::builds::{BuildJob, Error, UploadedFile, model::Target, store, version::Version};

const KEEP: usize = 3;
const PRUNE_LIMIT: Duration = Duration::from_mins(2);

pub(super) async fn prune(db: &PgPool, reach: Reach<'_>, target: Target) {
    let fault = match tokio::time::timeout(PRUNE_LIMIT, remove_stale(db, reach, target)).await {
        Ok(Ok(())) => return,
        Ok(Err(fault)) => fault,
        Err(_) => "time limit",
    };
    tracing::warn!(
        domain = "builds",
        fault,
        "publishing could not prune old versions"
    );
}

async fn remove_stale(db: &PgPool, reach: Reach<'_>, target: Target) -> Result<(), &'static str> {
    let database = |_| "database";
    let mut held: Vec<_> = store::unremoved_published(db, target)
        .await
        .map_err(database)?
        .into_iter()
        .filter_map(|(job, version)| Some((version.parse::<Version>().ok()?, job)))
        .collect();
    held.sort_unstable_by_key(|(version, _)| Reverse(*version));
    for (version, job) in held.into_iter().skip(KEEP).rev() {
        let files = store::files_of(db, job).await.map_err(database)?;
        if !is_live(reach, target, version).await?
            && delete_objects(reach, target, version, &files).await
        {
            store::mark_removed(db, job, reach.clock.now())
                .await
                .map_err(database)?;
        }
    }
    Ok(())
}

async fn is_live(reach: Reach<'_>, target: Target, version: Version) -> Result<bool, &'static str> {
    for folder in feed::platform_keys(target) {
        let object = format!("{folder}/latest.json");
        let get = reach.bucket.get(&object, reach.clock.now());
        let bytes = transfer::read_small(reach.http, get, "latest.json", feed::MAX_FEED_BYTES)
            .await
            .map_err(|stop| match stop {
                Stop::Unavailable(fault) => fault,
                _ => "invalid live feed",
            })?;
        let live = feed::published_version(&bytes).ok_or("invalid live feed")?;
        if version == live {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) async fn remove_failed(
    db: &PgPool,
    reach: Reach<'_>,
    job: &BuildJob,
) -> Result<(), Error> {
    if !store::is_published(db, job.target, &job.version.to_string()).await? {
        let files = store::files_of(db, job.id).await?;
        delete_objects(reach, job.target, job.version, &files).await;
    }
    Ok(())
}

async fn delete_objects(
    reach: Reach<'_>,
    target: Target,
    version: Version,
    files: &[UploadedFile],
) -> bool {
    let signature = feed::updater_name(target, files).map(feed::signature_name);
    let names = files
        .iter()
        .map(|file| file.name.as_str())
        .chain(signature.as_deref());
    let mut all = true;
    for name in names {
        let object = format!("{target}/{version}/{name}");
        let delete = reach.bucket.delete(&object, reach.clock.now());
        if transfer::send(reach.http, delete, Vec::new(), name)
            .await
            .is_err()
        {
            all = false;
            tracing::warn!(
                domain = "builds",
                fault = "bucket delete",
                "publishing could not remove a file"
            );
        }
    }
    all
}
