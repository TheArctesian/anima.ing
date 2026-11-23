//! The count and summary rows accepted in the daemon push transaction.

use daemon_core::wire::{ThreadSummaries, ThreadSummary, ThreadSummaryReceipt};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::ids::DeviceId;

pub(super) async fn record_count(
    tx: &mut PgConnection,
    device: DeviceId,
    batch: &ThreadSummaries,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "WITH written AS (
            INSERT INTO agent_threads.counts AS old (device_id, count, revision)
            VALUES ($1, $2, $3)
            ON CONFLICT (device_id) DO UPDATE SET count = EXCLUDED.count, revision = EXCLUDED.revision
            WHERE old.revision < EXCLUDED.revision
            RETURNING revision
        )
        SELECT revision FROM written
        UNION ALL
        SELECT revision FROM agent_threads.counts
        WHERE device_id = $1 AND NOT EXISTS (SELECT 1 FROM written)",
    )
    .bind(device)
    .bind(i64::from(batch.count))
    .bind(batch.count_revision)
    .fetch_one(tx)
    .await
}

pub(super) async fn record_summary(
    tx: &mut PgConnection,
    device: DeviceId,
    row: &ThreadSummary,
) -> Result<ThreadSummaryReceipt, sqlx::Error> {
    let (id, revision) = sqlx::query_as::<_, (Uuid, i64)>(
        "WITH written AS (
            INSERT INTO agent_threads.summaries AS old (
                id, device_id, runner_id, program, state, title, started_at, activity_at,
                repository, revision
            )
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $6, $7, $8, $9)
            ON CONFLICT (device_id, runner_id) DO UPDATE SET
                program = EXCLUDED.program, state = EXCLUDED.state, title = EXCLUDED.title,
                started_at = EXCLUDED.started_at, activity_at = EXCLUDED.activity_at,
                repository = EXCLUDED.repository, revision = EXCLUDED.revision
            WHERE old.revision < EXCLUDED.revision
            RETURNING id, revision
        )
        SELECT id, revision FROM written
        UNION ALL
        SELECT id, revision FROM agent_threads.summaries
        WHERE device_id = $1 AND runner_id = $2 AND NOT EXISTS (SELECT 1 FROM written)",
    )
    .bind(device)
    .bind(row.runner_id)
    .bind(&row.program)
    .bind(&row.state)
    .bind(&row.title)
    .bind(row.started_at)
    .bind(row.activity_at)
    .bind(row.repository.as_deref())
    .bind(row.revision)
    .fetch_one(tx)
    .await?;
    Ok(ThreadSummaryReceipt {
        runner_id: row.runner_id,
        id,
        revision,
    })
}
