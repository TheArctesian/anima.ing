//! The device binding and one-use encrypted inputs of a managed application.
use sqlx::PgConnection;
use uuid::Uuid;

use super::{DeviceId, Error, JobId};

pub(in crate::work::jobs) async fn bind(
    conn: &mut PgConnection,
    application: Uuid,
    device: DeviceId,
) -> Result<bool, Error> {
    sqlx::query(
        "INSERT INTO jobs.application_machines (application_id, device_id)
         VALUES ($1, $2) ON CONFLICT (application_id) DO NOTHING",
    )
    .bind(application)
    .bind(device)
    .execute(&mut *conn)
    .await?;
    bound(conn, application, device).await
}

pub(in crate::work::jobs) async fn bound(
    conn: &mut PgConnection,
    application: Uuid,
    device: DeviceId,
) -> Result<bool, Error> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM jobs.application_machines
         WHERE application_id = $1 AND device_id = $2)",
    )
    .bind(application)
    .bind(device)
    .fetch_one(conn)
    .await?)
}

pub(in crate::work::jobs) async fn insert(
    conn: &mut PgConnection,
    job: JobId,
    purpose: &str,
    sealed: &[u8],
) -> Result<(), Error> {
    sqlx::query("INSERT INTO jobs.application_files (job_id, purpose, sealed) VALUES ($1, $2, $3)")
        .bind(job)
        .bind(purpose)
        .bind(sealed)
        .execute(conn)
        .await?;
    Ok(())
}

pub(in crate::work::jobs) async fn take(
    conn: &mut PgConnection,
    job: JobId,
    purpose: &str,
) -> Result<Option<Vec<u8>>, Error> {
    Ok(sqlx::query_scalar(
        "DELETE FROM jobs.application_files WHERE job_id = $1 AND purpose = $2 RETURNING sealed",
    )
    .bind(job)
    .bind(purpose)
    .fetch_optional(conn)
    .await?)
}

pub(super) async fn clear(conn: &mut PgConnection, job: JobId) -> Result<(), Error> {
    sqlx::query("DELETE FROM jobs.application_files WHERE job_id = $1")
        .bind(job)
        .execute(conn)
        .await?;
    Ok(())
}
