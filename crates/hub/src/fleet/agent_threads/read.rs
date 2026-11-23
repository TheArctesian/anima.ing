use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::ids::DeviceId;

#[derive(sqlx::FromRow)]
pub(super) struct Summary {
    pub id: Uuid,
    pub device_id: DeviceId,
    pub program: String,
    pub state: String,
    pub title: String,
    pub started_at: DateTime<Utc>,
    pub activity_at: DateTime<Utc>,
    pub repository: Option<String>,
}

#[derive(sqlx::FromRow)]
pub(super) struct Entry {
    pub sequence: i64,
    pub content: serde_json::Value,
}

#[derive(Default)]
pub(super) struct Filter {
    pub device: Option<DeviceId>,
    pub program: Option<String>,
    pub project: Option<String>,
    pub state: Option<String>,
    pub after: Option<(DateTime<Utc>, Uuid)>,
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

pub(super) async fn summary(
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
