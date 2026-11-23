//! PRO-059 to PRO-063.

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgPool};

use crate::ids::{ProjectGroupId, ProjectId};

/// `project_count` counts projects with a repository that is not left out (PRO-064).
#[derive(Clone, Debug, sqlx::FromRow)]
pub(in crate::work::projects) struct GroupRow {
    pub id: ProjectGroupId,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub project_count: i64,
    pub project_ids: Vec<ProjectId>,
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub(in crate::work::projects) struct MovedRow {
    pub id: ProjectId,
    pub slug: String,
    pub before: Option<ProjectGroupId>,
}

const GROUP: &str = "SELECT g.id, g.name, g.created_at, \
    (SELECT count(*) FROM projects.projects p WHERE p.group_id = g.id AND EXISTS ( \
        SELECT 1 FROM projects.repositories r WHERE r.project_id = p.id AND r.left_out_at IS NULL \
    )) AS project_count, ARRAY(SELECT p.id FROM projects.projects p WHERE p.group_id = g.id ORDER BY p.id) AS project_ids \
    FROM projects.groups g";

pub(in crate::work::projects) async fn all(db: &PgPool) -> Result<Vec<GroupRow>, sqlx::Error> {
    sqlx::query_as(&format!(
        "{GROUP} ORDER BY lower(g.name) COLLATE \"C\", g.id"
    ))
    .fetch_all(db)
    .await
}

pub(in crate::work::projects) async fn of_projects(
    db: &PgPool,
    projects: &[ProjectId],
) -> Result<Vec<(ProjectId, GroupRow)>, sqlx::Error> {
    let pairs: Vec<(ProjectId, ProjectGroupId)> = sqlx::query_as(
        "SELECT id, group_id FROM projects.projects WHERE id = ANY($1) AND group_id IS NOT NULL",
    )
    .bind(projects)
    .fetch_all(db)
    .await?;
    let mut ids: Vec<ProjectGroupId> = Vec::new();
    for (_, group) in &pairs {
        if !ids.contains(group) {
            ids.push(*group);
        }
    }
    let groups: Vec<GroupRow> = sqlx::query_as(&format!("{GROUP} WHERE g.id = ANY($1)"))
        .bind(&ids)
        .fetch_all(db)
        .await?;
    Ok(pairs
        .into_iter()
        .filter_map(|(project, group)| {
            let row = groups.iter().find(|row| row.id == group)?;
            Some((project, row.clone()))
        })
        .collect())
}

pub(in crate::work::projects) async fn one(
    conn: &mut PgConnection,
    id: ProjectGroupId,
) -> Result<Option<GroupRow>, sqlx::Error> {
    sqlx::query_as(&format!("{GROUP} WHERE g.id = $1"))
        .bind(id)
        .fetch_optional(conn)
        .await
}

pub(in crate::work::projects) async fn lock(
    conn: &mut PgConnection,
    id: ProjectGroupId,
) -> Result<bool, sqlx::Error> {
    let found: Option<ProjectGroupId> =
        sqlx::query_scalar("SELECT id FROM projects.groups WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(conn)
            .await?;
    Ok(found.is_some())
}

pub(in crate::work::projects) async fn name_taken(
    conn: &mut PgConnection,
    name: &str,
    except: Option<ProjectGroupId>,
) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM projects.groups WHERE lower(name) = lower($1) \
         AND ($2::uuid IS NULL OR id <> $2))",
    )
    .bind(name)
    .bind(except)
    .fetch_one(conn)
    .await
}

pub(in crate::work::projects) async fn insert(
    conn: &mut PgConnection,
    (id, name): (ProjectGroupId, &str),
    now: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO projects.groups (id, name, created_at) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(name)
        .bind(now)
        .execute(conn)
        .await
        .map(drop)
}

pub(in crate::work::projects) async fn rename(
    conn: &mut PgConnection,
    id: ProjectGroupId,
    name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE projects.groups SET name = $2 WHERE id = $1")
        .bind(id)
        .bind(name)
        .execute(conn)
        .await
        .map(drop)
}

/// Its projects keep each repository and get no group (PRO-062).
pub(in crate::work::projects) async fn delete(
    conn: &mut PgConnection,
    id: ProjectGroupId,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM projects.groups WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await
        .map(drop)
}

pub(in crate::work::projects) async fn existing(
    conn: &mut PgConnection,
    projects: &[ProjectId],
) -> Result<Vec<ProjectId>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM projects.projects WHERE id = ANY($1)")
        .bind(projects)
        .fetch_all(conn)
        .await
}

pub(in crate::work::projects) async fn set_group(
    conn: &mut PgConnection,
    projects: &[ProjectId],
    group: Option<ProjectGroupId>,
) -> Result<Vec<MovedRow>, sqlx::Error> {
    sqlx::query_as(
        "UPDATE projects.projects p SET group_id = $2 FROM projects.projects old \
         WHERE p.id = old.id AND p.id = ANY($1) AND p.group_id IS DISTINCT FROM $2 \
         RETURNING p.id, p.slug, old.group_id AS before",
    )
    .bind(projects)
    .bind(group)
    .fetch_all(conn)
    .await
}

/// The source row serializes retries, including batches that arrive from different clients.
pub(in crate::work::projects) async fn import_group(
    conn: &mut PgConnection,
    source: uuid::Uuid,
    name: &str,
    now: DateTime<Utc>,
) -> Result<ProjectGroupId, sqlx::Error> {
    sqlx::query("INSERT INTO projects.group_imports (source) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(source)
        .execute(&mut *conn)
        .await?;
    let previous: Option<ProjectGroupId> = sqlx::query_scalar(
        "SELECT group_id FROM projects.group_imports WHERE source = $1 FOR UPDATE",
    )
    .bind(source)
    .fetch_one(&mut *conn)
    .await?;
    if let Some(id) = previous {
        return Ok(id);
    }
    let id = sqlx::query_scalar(
        "INSERT INTO projects.groups (id, name, created_at) VALUES ($1, $2, $3) \
         ON CONFLICT (lower(name)) DO UPDATE SET name = projects.groups.name RETURNING id",
    )
    .bind(ProjectGroupId::from(uuid::Uuid::now_v7()))
    .bind(name)
    .bind(now)
    .fetch_one(&mut *conn)
    .await?;
    sqlx::query("UPDATE projects.group_imports SET group_id = $2 WHERE source = $1")
        .bind(source)
        .bind(id)
        .execute(conn)
        .await?;
    Ok(id)
}

/// A receipt is recorded even for a project already assigned elsewhere.
pub(in crate::work::projects) async fn import_members(
    conn: &mut PgConnection,
    source: uuid::Uuid,
    projects: &[ProjectId],
) -> Result<Vec<ProjectId>, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO projects.group_import_members (source, project_id) \
         SELECT $1, id FROM unnest($2::uuid[]) AS id ORDER BY id \
         ON CONFLICT DO NOTHING RETURNING project_id",
    )
    .bind(source)
    .bind(projects)
    .fetch_all(conn)
    .await
}

pub(in crate::work::projects) async fn import_unassigned(
    conn: &mut PgConnection,
    projects: &[ProjectId],
    group: ProjectGroupId,
) -> Result<Vec<ProjectId>, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE projects.projects SET group_id = $2 WHERE id = ANY($1) AND group_id IS NULL RETURNING id")
        .bind(projects).bind(group).fetch_all(conn).await
}
