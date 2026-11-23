use daemon_core::wire::{ThreadEntry, ThreadSummaries, ThreadSummary, ThreadSummaryReceipt};
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::model::{Entry, Filter, Summary};
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

pub(super) async fn list(
    pool: &PgPool,
    filter: Filter,
    limit: i64,
) -> Result<Vec<Summary>, sqlx::Error> {
    let (at, id) = filter.after.unzip();
    sqlx::query_as(
        "SELECT id, device_id, program, state, title, started_at, activity_at, repository
         FROM agent_threads.summaries
         WHERE ($1::uuid IS NULL OR device_id = $1)
           AND ($2::text IS NULL OR program = $2)
           AND ($3::text IS NULL OR repository = $3)
           AND ($4::text IS NULL OR state = $4)
           AND ($5::timestamptz IS NULL OR (activity_at, id) < ($5, $6))
         ORDER BY activity_at DESC, id DESC LIMIT $7",
    )
    .bind(filter.device)
    .bind(filter.program)
    .bind(filter.project)
    .bind(filter.state)
    .bind(at)
    .bind(id)
    .bind(limit)
    .fetch_all(pool)
    .await
}

async fn summary(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
) -> Result<Option<Summary>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, device_id, program, state, title, started_at, activity_at, repository
         FROM agent_threads.summaries WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await
}

pub(super) async fn entries<'e>(
    db: impl sqlx::Executor<'e, Database = Postgres>,
    id: Uuid,
    after: i64,
) -> Result<Vec<Entry>, sqlx::Error> {
    sqlx::query_as(
        "SELECT sequence, content FROM agent_threads.entries
         WHERE thread_id = $1 AND sequence > $2 ORDER BY sequence LIMIT 64",
    )
    .bind(id)
    .bind(after)
    .fetch_all(db)
    .await
}

pub(super) async fn snapshot(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<(Summary, Vec<Entry>)>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let Some(summary) = summary(&mut tx, id).await? else {
        return Ok(None);
    };
    let entries = sqlx::query_as(
        "SELECT sequence, content FROM (
            SELECT sequence, content FROM agent_threads.entries
            WHERE thread_id = $1 ORDER BY sequence DESC LIMIT 64
         ) AS recent ORDER BY sequence",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some((summary, entries)))
}

pub(super) async fn last_sequence(conn: &mut PgConnection, id: Uuid) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT COALESCE(max(sequence), 0) FROM agent_threads.entries WHERE thread_id = $1",
    )
    .bind(id)
    .fetch_one(conn)
    .await
}

pub(super) async fn insert_entry(
    conn: &mut PgConnection,
    thread: Uuid,
    entry: &ThreadEntry,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO agent_threads.entries (thread_id, sequence, content) VALUES ($1, $2, $3)",
    )
    .bind(thread)
    .bind(entry.sequence)
    .bind(&entry.content)
    .execute(conn)
    .await?;
    Ok(())
}
