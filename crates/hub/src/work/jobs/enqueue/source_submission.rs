//! A signed-in source-build request gets one durable job across retries.
use super::{Asker, Error, Job, NewJob, Queue, ended, insert_queued, queued, store, validate};
use crate::work::jobs::{SourceSubmission, placement};
use chrono::{DateTime, Utc};
use daemon_core::wire::WorkType;

pub(crate) async fn enqueue_source(
    queue: Queue<'_>,
    (wanted, source): (NewJob, SourceSubmission),
    asker: Asker,
    now: DateTime<Utc>,
) -> Result<Job, Error> {
    let wanted = wanted.bind_source_target()?;
    validate(&wanted)?;
    if source.request.is_nil()
        || wanted.work_type != WorkType::Build
        || wanted.parameters["recipe"] != "source_image_v1"
        || source.checkout.is_some() != placement::needs_private_checkout(&wanted)
    {
        return Err(Error::Invalid("the source build submission is invalid"));
    }
    let mut tx = queue.db.begin().await?;
    let outcome = async {
        if let Some(held) = store::checkout::locked_submission(&mut tx, source.request).await? {
            if held.operator != asker.0.id
                || held.repository != source.repository
                || held.checkout()? != source.checkout
            {
                return Err(conflict());
            }
            let job = store::job_locked(&mut tx, held.job)
                .await?
                .ok_or(Error::Unreadable)?;
            return if job.asked == wanted {
                Ok(job)
            } else {
                Err(conflict())
            };
        }
        let job = queued(wanted, now);
        let operator = asker.0.id;
        insert_queued(&mut tx, &job, asker, now).await?;
        store::checkout::insert_submission(&mut tx, (job.id, operator), source).await?;
        Ok(job)
    }
    .await;
    ended(tx, outcome).await
}

fn conflict() -> Error {
    Error::Invalid("the build request already belongs to another submission")
}
