//! All SQL on `compute.copies`: I/O only. Writes take the caller's connection, so the copy and
//! its job are written in one transaction.

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgExecutor};
use uuid::Uuid;

use super::super::library::Format;
use super::{
    Error,
    model::{Copy, Origin},
};
use crate::ids::{CopyId, DeviceId, JobId};

const SELECT: &str = "SELECT id, device_id, variant_id, model, tag, format, quantisation, \
    repository, revision, size_bytes, files, pull_job_id, delete_job_id, created_at, origin, \
    found FROM compute.copies ";

#[derive(sqlx::FromRow)]
struct CopyRow {
    id: CopyId,
    device_id: DeviceId,
    variant_id: Option<Uuid>,
    model: String,
    tag: String,
    format: String,
    quantisation: String,
    repository: String,
    revision: String,
    size_bytes: i64,
    files: i32,
    pull_job_id: JobId,
    delete_job_id: Option<JobId>,
    created_at: DateTime<Utc>,
    origin: String,
    found: Option<String>,
}

impl TryFrom<CopyRow> for Copy {
    type Error = Error;

    fn try_from(row: CopyRow) -> Result<Self, Error> {
        Ok(Self {
            id: row.id,
            device: row.device_id,
            variant: row.variant_id,
            model: row.model,
            tag: row.tag,
            format: Format::from_stored(&row.format).ok_or(Error::Unreadable)?,
            quantisation: row.quantisation,
            repository: row.repository,
            revision: row.revision,
            size_bytes: u64::try_from(row.size_bytes).map_err(|_| Error::Unreadable)?,
            files: u32::try_from(row.files).map_err(|_| Error::Unreadable)?,
            pull_job: row.pull_job_id,
            delete_job: row.delete_job_id,
            created_at: row.created_at,
            origin: Origin::from_stored(&row.origin).ok_or(Error::Unreadable)?,
            found: row.found,
        })
    }
}

/// Each copy, in the order of the machine, the model and the tag.
pub(super) async fn all(db: impl PgExecutor<'_>) -> Result<Vec<Copy>, Error> {
    let sql = format!("{SELECT}ORDER BY device_id, model, tag");
    let rows: Vec<CopyRow> = sqlx::query_as(&sql).fetch_all(db).await?;
    rows.into_iter().map(Copy::try_from).collect()
}

/// The copy, locked until the transaction ends.
pub(super) async fn locked(conn: &mut PgConnection, id: CopyId) -> Result<Option<Copy>, Error> {
    let sql = format!("{SELECT}WHERE id = $1 FOR UPDATE");
    let row: Option<CopyRow> = sqlx::query_as(&sql).bind(id).fetch_optional(conn).await?;
    row.map(Copy::try_from).transpose()
}

/// The copy of `model:tag` on the machine, locked until the transaction ends.
pub(super) async fn at_locked(
    conn: &mut PgConnection,
    device: DeviceId,
    (model, tag): (&str, &str),
) -> Result<Option<Copy>, Error> {
    let sql = format!(
        "{SELECT}WHERE device_id = $1 AND model = $2 AND tag = $3 AND origin = 'pulled' FOR UPDATE"
    );
    let row: Option<CopyRow> = sqlx::query_as(&sql)
        .bind(device)
        .bind(model)
        .bind(tag)
        .fetch_optional(conn)
        .await?;
    row.map(Copy::try_from).transpose()
}

pub(super) async fn insert(conn: &mut PgConnection, copy: &Copy) -> Result<(), Error> {
    let size = i64::try_from(copy.size_bytes).map_err(|_| Error::Unreadable)?;
    let files = i32::try_from(copy.files).map_err(|_| Error::Unreadable)?;
    sqlx::query(
        "INSERT INTO compute.copies (id, device_id, variant_id, model, tag, format, \
         quantisation, repository, revision, size_bytes, files, pull_job_id, delete_job_id, \
         created_at, origin, found) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)",
    )
    .bind(copy.id)
    .bind(copy.device)
    .bind(copy.variant)
    .bind(&copy.model)
    .bind(&copy.tag)
    .bind(copy.format.as_str())
    .bind(&copy.quantisation)
    .bind(&copy.repository)
    .bind(&copy.revision)
    .bind(size)
    .bind(files)
    .bind(copy.pull_job)
    .bind(copy.delete_job)
    .bind(copy.created_at)
    .bind(copy.origin.as_str())
    .bind(&copy.found)
    .execute(conn)
    .await?;
    Ok(())
}

/// The found copy of the model with the key `found` on the machine, locked until the
/// transaction ends.
pub(super) async fn found_locked(
    conn: &mut PgConnection,
    device: DeviceId,
    found: &str,
) -> Result<Option<Copy>, Error> {
    let sql =
        format!("{SELECT}WHERE device_id = $1 AND found = $2 AND origin = 'found' FOR UPDATE");
    let row: Option<CopyRow> = sqlx::query_as(&sql)
        .bind(device)
        .bind(found)
        .fetch_optional(conn)
        .await?;
    row.map(Copy::try_from).transpose()
}

/// Sets the jobs of the copy: a new pull clears the removal that failed.
pub(super) async fn set_jobs(
    conn: &mut PgConnection,
    id: CopyId,
    (pull, delete): (JobId, Option<JobId>),
) -> Result<(), Error> {
    sqlx::query("UPDATE compute.copies SET pull_job_id = $2, delete_job_id = $3 WHERE id = $1")
        .bind(id)
        .bind(pull)
        .bind(delete)
        .execute(conn)
        .await?;
    Ok(())
}

pub(super) async fn forget(conn: &mut PgConnection, id: CopyId) -> Result<(), Error> {
    sqlx::query("DELETE FROM compute.copies WHERE id = $1")
        .bind(id)
        .execute(conn)
        .await?;
    Ok(())
}
