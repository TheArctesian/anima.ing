use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use super::{JobRow, SELECT_JOB};
use crate::{
    ids::{DeviceId, JobId, ProjectId},
    work::jobs::{Error, Job},
};

pub(in crate::work::jobs) async fn page(
    conn: &mut PgConnection,
    limit: i64,
    after: Option<(DateTime<Utc>, JobId)>,
    state: Option<&str>,
) -> Result<Vec<Job>, Error> {
    let (time, id) = after.unzip();
    let rows: Vec<JobRow> = sqlx::query_as(&format!(
        "{SELECT_JOB} WHERE ($1::timestamptz IS NULL OR (created_at, id) < ($1, $2))
         AND ($3::text IS NULL OR state = $3)
         ORDER BY created_at DESC, id DESC LIMIT $4",
    ))
    .bind(time)
    .bind(id)
    .bind(state)
    .bind(limit)
    .fetch_all(conn)
    .await?;
    rows.into_iter().map(Job::try_from).collect()
}

pub(in crate::work::jobs) async fn lock_project(
    conn: &mut PgConnection,
    project: ProjectId,
) -> Result<bool, Error> {
    let found: Option<ProjectId> =
        sqlx::query_scalar("SELECT id FROM projects.projects WHERE id = $1 FOR UPDATE")
            .bind(project)
            .fetch_optional(conn)
            .await?;
    Ok(found.is_some())
}

pub(in crate::work::jobs) async fn replace_devices(
    conn: &mut PgConnection,
    project: ProjectId,
    devices: &[DeviceId],
) -> Result<(), Error> {
    sqlx::query("DELETE FROM jobs.project_devices WHERE project_id = $1")
        .bind(project)
        .execute(&mut *conn)
        .await?;
    sqlx::query(
        "INSERT INTO jobs.project_devices (project_id, device_id)
         SELECT $1, device FROM unnest($2::uuid[]) AS offered(device)",
    )
    .bind(project)
    .bind(devices)
    .execute(conn)
    .await?;
    Ok(())
}
