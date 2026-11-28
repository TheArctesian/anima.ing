//! Publishing (BLD-060 to BLD-070, D-24 to D-28, R-117).
//!
//! The publisher runs in the upkeep sweep (D-25), never inside a request. It holds a job instead
//! of a row lock across the network calls, checks the hold before each write, and records the
//! end only if its try is still the job's latest. A job is taken only once every upload URL of
//! it expired, so its files cannot change after they are checked (D-28).

mod bucket;
mod built;
mod feed;
mod files;
mod pruning;
mod run;
mod signature;
mod sigv4;
#[cfg(test)]
mod tests;
mod transfer;
mod tries;
mod updater_key;

use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::Client;
use sqlx::{PgConnection, PgPool};

pub use bucket::{Bucket, BucketError, UploadUrl};
pub use built::{
    Built, BuiltError, BuiltFile, BuiltPublisher, CheckedBuilt, ReleaseError, check_built,
    release_command, sign_built,
};
use run::{Outcome, Reach};
pub use updater_key::{UpdaterKey, UpdaterKeyError};

use super::{
    BuildJob, BuildQueue, Builder, Error, FieldError, JobUpdate, UploadedFile, jobs, queue, store,
    upload,
};
use crate::{clock::Clock, config::Releases, ids::BuildJobId};

/// How often the upkeep runs the publisher (D-25).
pub const PUBLISH_SWEEP: Duration = Duration::from_secs(10);

const RUN_LIMIT: Duration = Duration::from_mins(7);

// A try ends at least 3 minutes before its hold, so a write it sent near the end lands before a
// later try can take the job (D-25).
const _: () = assert!(RUN_LIMIT.as_secs() + 180 <= tries::HOLD.num_seconds().unsigned_abs());

const MAX_PER_SWEEP: usize = 4;

/// The reason of a job handed to a hub whose publishing is off (D-25).
const PUBLISHING_OFF: &str = "Publishing is off on this hub: no release bucket, or no usable updater key (the hub log says which).";

/// `None`, publishing off, when the updater key cannot be used (R-117). The warning names the
/// check that failed, never a value (LOG-007).
///
/// # Errors
///
/// A token file of the bucket that cannot be read or is empty.
pub fn open_bucket(releases: &Releases) -> Result<Option<Bucket>, BucketError> {
    match UpdaterKey::open(releases) {
        Ok(key) => Bucket::open(releases, key).map(Some),
        Err(error) => {
            tracing::warn!(domain = "builds", %error, "publishing is off: the updater key cannot be used");
            Ok(None)
        }
    }
}

/// A builder's request for the upload URLs of the files of a job it holds (BLD-060).
pub struct Grant<'a> {
    pub builder: &'a Builder,
    pub job: BuildJobId,
    pub files: &'a [UploadedFile],
}

/// Presigned `PUT` URLs for an `uploading` job its builder holds; extends the lease (BLD-060,
/// BLD-061, D-28). The files become the job's grant: the `uploaded` report names exactly them (D-26).
///
/// # Errors
///
/// `Conflict` when the job is not `uploading`; `Validation` for a bad file list; as `report` otherwise.
pub async fn upload_urls(
    db: &PgPool,
    bucket: &Bucket,
    grant: Grant<'_>,
    now: DateTime<Utc>,
) -> Result<Vec<UploadUrl>, Error> {
    let files = upload::files(grant.files).map_err(|e| Error::Validation(vec![e]))?;
    let mut tx = db.begin().await?;
    let outcome = granted(&mut tx, &grant, &files, now).await;
    let (next, kinds) = jobs::ended(tx, outcome).await?;
    let folder = format!("{}/{}", next.target, next.version);
    Ok(files
        .iter()
        .zip(kinds)
        .map(|(file, kind)| bucket.upload_url(&folder, (file, kind), now))
        .collect())
}

async fn granted(
    conn: &mut PgConnection,
    grant: &Grant<'_>,
    files: &[UploadedFile],
    now: DateTime<Utc>,
) -> Result<(BuildJob, Vec<&'static str>), Error> {
    let next = jobs::extend_uploading(conn, grant.builder, grant.job, now).await?;
    let kinds = files
        .iter()
        .map(|file| files::content_type(next.target, next.version, &file.name))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            let message = format!("A {} job uploads only its release files.", next.target);
            Error::Validation(vec![FieldError::new("files", message)])
        })?;
    store::replace_files(conn, next.id, files).await?;
    store::save(conn, &next).await?;
    store::extend_upload_expiry(conn, next.id, now + bucket::UPLOAD_URL_LIFE).await?;
    Ok((next, kinds))
}

/// Publishes the ready jobs one after the other and returns how many ended a try (D-25, D-28).
///
/// # Errors
///
/// `Database` or `Events` when a take or an end cannot be written; the job is then taken again
/// after its hold.
pub async fn publish_ready(
    build_queue: BuildQueue<'_>,
    bucket: &Bucket,
    http: &Client,
    clock: &dyn Clock,
) -> Result<usize, Error> {
    let reach = Reach {
        bucket,
        http,
        clock,
    };
    let mut ended = 0;
    while ended < MAX_PER_SWEEP {
        let Some((tried, work)) = tries::take(build_queue.db, clock.now()).await? else {
            break;
        };
        let outcome = match work {
            Some(work) => {
                let publish = run::publish(reach, build_queue.db, tried, &work);
                tokio::time::timeout(RUN_LIMIT, publish)
                    .await
                    .unwrap_or(Outcome::Unavailable("time limit"))
            }
            None => Outcome::Spent,
        };
        tries::finish(build_queue, reach, tried, outcome).await?;
        ended += 1;
    }
    Ok(ended)
}

/// Fails every job handed to the publisher while publishing is off, so none waits for ever (D-25).
///
/// # Errors
///
/// `Database` or `Events` when the failures cannot be written.
pub async fn publishing_off(
    build_queue: BuildQueue<'_>,
    now: DateTime<Utc>,
) -> Result<usize, Error> {
    let mut tx = build_queue.db.begin().await?;
    let mut failed = Vec::new();
    for job in store::handed_over(&mut tx).await? {
        let next = queue::publish_failed(&job, PUBLISHING_OFF, now);
        store::save(&mut tx, &next).await?;
        jobs::record_failure(&mut tx, build_queue.bus, &next).await?;
        failed.push(next);
    }
    tx.commit().await?;
    let count = failed.len();
    for job in failed {
        build_queue.updates.publish(JobUpdate {
            job,
            lines: Vec::new(),
        });
    }
    Ok(count)
}
