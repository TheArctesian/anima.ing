//! The publisher's tries (D-25, BLD-070). Each step reads the clock again, so a try that ran
//! for minutes records the time it ended.

use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, PgPool, Postgres, Transaction};

use super::{
    feed,
    run::{Outcome, Reach, Tried, Work},
    transfer,
};
use crate::fleet::{
    builds::{BuildJob, BuildQueue, Error, JobState, JobUpdate, Missing, jobs, queue, store},
    events::{self, ActorKind, Bus, NewEvent, Severity, Source},
};

/// A hub that stopped mid-publish is taken over after this.
pub(super) const HOLD: TimeDelta = TimeDelta::minutes(10);

const RETRY: TimeDelta = TimeDelta::minutes(1);

/// A job is taken only this long after its last upload URL expired, for clock skew (D-28).
const SKEW_MARGIN: TimeDelta = TimeDelta::minutes(2);

const MAX_ATTEMPTS: i32 = 3;

const STOPPED: &str = "Publishing stopped 3 times without an end.";

const UNREACHABLE: &str = "The release bucket could not be reached 3 times.";

/// Tries ran out after a feed write was sent, and no try could read `latest.json` (D-25).
const FEED_UNKNOWN: &str = "Feed state unknown, check latest.json.";

/// The work is `None` when the job's tries are spent, so the caller fails it (D-25, D-28).
pub(super) async fn take(
    db: &PgPool,
    now: DateTime<Utc>,
) -> Result<Option<(Tried, Option<Work>)>, Error> {
    let mut tx = db.begin().await?;
    let Some((job, attempts)) = store::next_publishable(&mut tx, now, now - SKEW_MARGIN).await?
    else {
        return Ok(None);
    };
    if attempts >= MAX_ATTEMPTS {
        tx.commit().await?;
        let tried = Tried {
            job: job.id,
            attempt: attempts,
        };
        return Ok(Some((tried, None)));
    }
    let attempt = attempts + 1;
    store::hold(&mut tx, job.id, Some(attempt), now + HOLD).await?;
    let files = store::files_of(&mut *tx, job.id).await?;
    let feed_sent = store::feed_sent(&mut *tx, job.id).await?;
    let feed = store::feed_of(&mut *tx, job.id).await?.unwrap_or_default();
    let published = store::published_versions(&mut *tx, job.target).await?;
    tx.commit().await?;
    let work = Work {
        target: job.target,
        version: job.version,
        files,
        feed,
        floor: published.iter().filter_map(|v| v.parse().ok()).max(),
        feed_sent,
        signed_at: job.times.uploading.unwrap_or(job.created_at),
    };
    Ok(Some((
        Tried {
            job: job.id,
            attempt,
        },
        Some(work),
    )))
}

enum End {
    /// The feed folders and the files changed after the check.
    Publish(&'static [&'static str], Vec<String>),
    Fail(String),
    Retry,
    Leave,
}

fn end_of(outcome: Outcome, attempt: i32, feed_sent: bool) -> End {
    let gave_up =
        |reason: &str| End::Fail((if feed_sent { FEED_UNKNOWN } else { reason }).to_owned());
    match outcome {
        Outcome::Published(folders, changed) => End::Publish(folders, changed),
        Outcome::Refused(reason) => End::Fail(reason),
        Outcome::TakenOver => End::Leave,
        Outcome::Spent => gave_up(STOPPED),
        Outcome::Unavailable(fault) => {
            tracing::warn!(
                domain = "builds",
                fault,
                attempt,
                "publishing could not reach the bucket"
            );
            if attempt < MAX_ATTEMPTS {
                End::Retry
            } else {
                gave_up(UNREACHABLE)
            }
        }
    }
}

pub(super) async fn finish(
    build_queue: BuildQueue<'_>,
    reach: Reach<'_>,
    tried: Tried,
    outcome: Outcome,
) -> Result<(), Error> {
    let mut tx = build_queue.db.begin().await?;
    let job = store::job_locked(&mut tx, tried.job)
        .await?
        .ok_or(Error::NotFound(Missing::Job(tried.job)))?;
    let current = store::publish_attempt(&mut *tx, tried.job).await?;
    // A job failed while publishing was off keeps its last attempt, so the state counts too.
    let held = job.state == JobState::Uploading && current == Some(tried.attempt);
    let feed_sent = store::feed_sent(&mut *tx, tried.job).await?;
    let end = if held {
        end_of(outcome, tried.attempt, feed_sent)
    } else {
        End::Leave
    };
    let now = reach.clock.now();
    let Some(next) = record(tx, build_queue.bus, (&job, end), now).await? else {
        return Ok(());
    };
    build_queue.updates.publish(JobUpdate {
        job: next.clone(),
        lines: Vec::new(),
    });
    // A feed that was sent may name the files, so they stay (D-25).
    if next.state == JobState::Failed && !feed_sent {
        remove_objects(build_queue.db, reach, &next).await?;
    }
    Ok(())
}

/// `tx` is always committed or rolled back here: a dropped transaction rolls back later, from a
/// task, so its row lock could outlive the try and the next take would skip the job (D-25).
async fn record(
    mut tx: Transaction<'_, Postgres>,
    bus: &Bus,
    (job, end): (&BuildJob, End),
    now: DateTime<Utc>,
) -> Result<Option<BuildJob>, Error> {
    let next = match end {
        End::Leave => {
            tx.rollback().await?;
            return Ok(None);
        }
        End::Retry => {
            store::hold(&mut tx, job.id, None, now + RETRY).await?;
            tx.commit().await?;
            return Ok(None);
        }
        End::Publish(folders, changed) => {
            let next = queue::published(job, now);
            store::save(&mut tx, &next).await?;
            store::set_written(&mut tx, job.id, folders).await?;
            let event = published_event(job);
            events::record(&mut tx, bus, now, event).await?;
            record_changed(&mut tx, bus, now, (job, &changed)).await?;
            next
        }
        End::Fail(reason) => {
            let next = queue::publish_failed(job, &reason, now);
            store::save(&mut tx, &next).await?;
            jobs::record_failure(&mut tx, bus, &next).await?;
            next
        }
    };
    tx.commit().await?;
    Ok(Some(next))
}

/// A failed build is never in the bucket for people (BLD-070).
async fn remove_objects(db: &PgPool, reach: Reach<'_>, job: &BuildJob) -> Result<(), Error> {
    let version = job.version.to_string();
    // A version is built once (D-27); this check still keeps a published file safe.
    if store::is_published(db, job.target, &version).await? {
        return Ok(());
    }
    let files = store::files_of(db, job.id).await?;
    // The hub's own `.sig` of the updater file goes too (R-117).
    let signature = feed::updater_name(job.target, &files).map(feed::signature_name);
    let names = files.iter().map(|file| file.name.clone()).chain(signature);
    for name in names {
        let object = format!("{}/{version}/{name}", job.target);
        let delete = reach.bucket.delete(&object, reach.clock.now());
        if transfer::send(reach.http, delete, Vec::new(), &name)
            .await
            .is_err()
        {
            tracing::warn!(
                domain = "builds",
                fault = "bucket delete",
                "publishing could not remove a file of a failed job"
            );
        }
    }
    Ok(())
}

/// BLD-082.
fn published_event(job: &BuildJob) -> NewEvent {
    NewEvent {
        severity: Severity::Info,
        source: Source::Hub,
        device: None,
        message: format!("Anima {} for {} is published.", job.version, job.target),
        actor: ActorKind::Hub,
    }
}

/// D-28.
async fn record_changed(
    tx: &mut PgConnection,
    bus: &Bus,
    now: DateTime<Utc>,
    (job, changed): (&BuildJob, &[String]),
) -> Result<(), Error> {
    if changed.is_empty() {
        return Ok(());
    }
    let event = NewEvent {
        severity: Severity::Warning,
        source: Source::Hub,
        device: None,
        message: format!(
            "Anima {} for {} is published, but {} changed in the release bucket after the hub \
             checked it. Check that the hub's clock keeps time (NTP).",
            job.version,
            job.target,
            changed.join(" and ")
        ),
        actor: ActorKind::Hub,
    };
    events::record(tx, bus, now, event).await?;
    Ok(())
}
