use sqlx::PgConnection;

use super::{job_locked, stored, word};
use crate::work::jobs::{Error, Job, ModelBudget, placement::Offer};

pub(in crate::work::jobs) async fn next(
    conn: &mut PgConnection,
    (offer, exclusive): (&Offer, bool),
    (reducing, ordinary): (&[&str], &[&str]),
    budget: Option<ModelBudget>,
) -> Result<Option<(Job, String)>, Error> {
    if reducing.is_empty() && ordinary.is_empty() {
        return Ok(None);
    }
    let budget = budget.unwrap_or(ModelBudget {
        memory_bytes: 0,
        disk_bytes: 0,
    });
    let advertised = |bytes| {
        // A model-only poll reports zero where no ordinary job capacity is advertised.
        if exclusive && bytes == 0 {
            i64::MAX
        } else {
            stored(bytes).unwrap_or(i64::MAX)
        }
    };
    let selected = sqlx::query_as(
        "SELECT id, actor FROM jobs.jobs
         WHERE kind = 'model' AND state = 'queued'
            AND (work_type = ANY($1) OR (work_type = ANY($8)
                AND limit_memory_bytes <= $9 AND limit_disk_bytes <= $10))
            AND (machine_id = $2 OR (machine_id IS NULL AND NOT $3))
            AND $2 <> ALL(refused_by)
            AND (os IS NULL OR os = $4) AND (architecture IS NULL OR architecture = $5)
            AND limit_memory_bytes <= $6 AND limit_disk_bytes <= $7
         ORDER BY CASE work_type WHEN 'model_drain' THEN 0 WHEN 'server_stop' THEN 1
            WHEN 'model_unload' THEN 2 ELSE 3 END, created_at, id
         LIMIT 1 FOR UPDATE SKIP LOCKED",
    )
    .bind(reducing)
    .bind(offer.device)
    .bind(offer.excluded)
    .bind(offer.os.as_ref().map(word).transpose()?)
    .bind(offer.architecture.as_ref().map(word).transpose()?)
    .bind(advertised(offer.memory_bytes))
    .bind(advertised(offer.disk_bytes))
    .bind(ordinary)
    .bind(stored(budget.memory_bytes).unwrap_or(i64::MAX))
    .bind(stored(budget.disk_bytes).unwrap_or(i64::MAX))
    .fetch_optional(&mut *conn)
    .await?;
    let Some((id, actor)) = selected else {
        return Ok(None);
    };
    let job = job_locked(conn, id).await?.ok_or(Error::Unreadable)?;
    Ok(Some((job, actor)))
}
