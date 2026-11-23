//! The operator's choices (PRO-002, PRO-004).

use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use crate::ids::{ProjectId, RepositoryId, ResourceId, SourceAccountId};

#[derive(Clone, Copy, Debug, sqlx::FromRow)]
pub(in crate::work::projects) struct Chosen {
    pub project_id: ProjectId,
    pub account_id: Option<SourceAccountId>,
    pub left_out: bool,
}

const CHOSEN: &str = "SELECT project_id, account_id, left_out_at IS NOT NULL AS left_out \
    FROM projects.repositories WHERE id = $1";

pub(in crate::work::projects) async fn repository(
    conn: &mut PgConnection,
    id: RepositoryId,
) -> Result<Option<Chosen>, sqlx::Error> {
    sqlx::query_as(&format!("{CHOSEN} FOR UPDATE"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

/// Not locked: a choice reads it before it locks the account row, in the sync's lock order
/// (account, then repository).
pub(in crate::work::projects) async fn peek(
    conn: &mut PgConnection,
    id: RepositoryId,
) -> Result<Option<Chosen>, sqlx::Error> {
    sqlx::query_as(CHOSEN).bind(id).fetch_optional(conn).await
}

pub(in crate::work::projects) async fn set_left_out(
    conn: &mut PgConnection,
    id: RepositoryId,
    at: Option<DateTime<Utc>>,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE projects.repositories SET left_out_at = $2 WHERE id = $1")
        .bind(id)
        .bind(at)
        .execute(conn)
        .await
        .map(drop)
}

pub(in crate::work::projects) async fn resource(
    conn: &mut PgConnection,
    id: ResourceId,
) -> Result<Option<(ProjectId, String, String)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT project_id, kind, state FROM projects.resources WHERE id = $1 FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(conn)
    .await
}

pub(in crate::work::projects) async fn confirm(
    conn: &mut PgConnection,
    id: ResourceId,
    at: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE projects.resources SET state = 'confirmed', confirmed_at = $2, \
         confirmed_by = 'operator' WHERE id = $1",
    )
    .bind(id)
    .bind(at)
    .execute(conn)
    .await
    .map(drop)
}

pub(in crate::work::projects) async fn slug(
    conn: &mut PgConnection,
    id: ProjectId,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar("SELECT slug FROM projects.projects WHERE id = $1")
        .bind(id)
        .fetch_one(conn)
        .await
}

pub(in crate::work::projects) async fn repository_slug(
    conn: &mut PgConnection,
    id: RepositoryId,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT p.slug FROM projects.repositories r \
         JOIN projects.projects p ON p.id = r.project_id WHERE r.id = $1",
    )
    .bind(id)
    .fetch_one(conn)
    .await
}
