//! Repository binding, one-release ledger and durable revocation queue.
use chrono::{DateTime, Utc};
use sqlx::PgConnection;

use crate::ids::{DeviceId, JobId, OperatorId, ProjectId, RepositoryId, SourceAccountId};
use uuid::Uuid;

use super::super::{CheckoutSource, Error, SourceSubmission};

#[derive(sqlx::FromRow)]
pub(in crate::work::jobs) struct Submission {
    pub operator: OperatorId,
    pub repository: RepositoryId,
    pub job: JobId,
    checkout_account: Option<SourceAccountId>,
    checkout_repository: Option<String>,
}

impl Submission {
    pub(in crate::work::jobs) fn checkout(&self) -> Result<Option<CheckoutSource>, Error> {
        self.checkout_account
            .zip(self.checkout_repository.as_deref())
            .map(|(account, external)| checkout_source(account, external))
            .transpose()
    }
}

pub(in crate::work::jobs) async fn locked_submission(
    conn: &mut PgConnection,
    request: Uuid,
) -> Result<Option<Submission>, Error> {
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended('source build submission:' || $1::uuid::text, 0))")
        .bind(request).execute(&mut *conn).await?;
    Ok(sqlx::query_as(
        "SELECT operator,repository,job,checkout_account,checkout_repository
         FROM jobs.source_build_submissions WHERE request=$1",
    )
    .bind(request)
    .fetch_optional(conn)
    .await?)
}

pub(in crate::work::jobs) async fn insert_submission(
    conn: &mut PgConnection,
    (job, operator): (JobId, OperatorId),
    source: SourceSubmission,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO jobs.source_build_submissions
         (request,operator,repository,job,checkout_account,checkout_repository) VALUES($1,$2,$3,$4,$5,$6)",
    )
    .bind(source.request).bind(operator).bind(source.repository).bind(job)
    .bind(source.checkout.map(|bound| bound.account))
    .bind(source.checkout.map(|bound| bound.forge_repository.to_string()))
    .execute(conn).await?;
    Ok(())
}

pub(in crate::work::jobs) async fn locked_pin(
    conn: &mut PgConnection,
    project: ProjectId,
) -> Result<Option<DeviceId>, Error> {
    Ok(sqlx::query_scalar(
        "SELECT device_id FROM jobs.project_machines WHERE project_id=$1 FOR SHARE",
    )
    .bind(project)
    .fetch_optional(conn)
    .await?)
}

pub(in crate::work::jobs) async fn source(
    conn: &mut PgConnection,
    job: JobId,
) -> Result<Option<(RepositoryId, CheckoutSource)>, Error> {
    let row: Option<(RepositoryId, SourceAccountId, String)> = sqlx::query_as(
        "SELECT repository,checkout_account,checkout_repository FROM jobs.source_build_submissions
         WHERE job=$1 AND checkout_account IS NOT NULL",
    )
    .bind(job)
    .fetch_optional(conn)
    .await?;
    row.map(|(repository, account, external)| {
        Ok((repository, checkout_source(account, &external)?))
    })
    .transpose()
}

fn checkout_source(account: SourceAccountId, external: &str) -> Result<CheckoutSource, Error> {
    let forge_repository = external.parse().map_err(|_| Error::Unreadable)?;
    Ok(CheckoutSource {
        account,
        forge_repository,
    })
}

pub(in crate::work) async fn reserve(
    conn: &mut PgConnection,
    (job, attempt, device): (JobId, u32, DeviceId),
) -> Result<bool, Error> {
    let attempt = i32::try_from(attempt).map_err(|_| Error::Unreadable)?;
    let result = sqlx::query(
        "INSERT INTO jobs.repository_token_releases(job,attempt,device)
         VALUES($1,$2,$3) ON CONFLICT DO NOTHING",
    )
    .bind(job)
    .bind(attempt)
    .bind(device)
    .execute(conn)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub(in crate::work) async fn save(
    conn: &mut PgConnection,
    (job, attempt, device): (JobId, u32, DeviceId),
    (sealed, expires_at, released_at): (&[u8], DateTime<Utc>, Option<DateTime<Utc>>),
) -> Result<(), Error> {
    let attempt = i32::try_from(attempt).map_err(|_| Error::Unreadable)?;
    let wrote = sqlx::query(
        "UPDATE jobs.repository_token_releases SET sealed=$4, expires_at=$5, released_at=$6
         WHERE job=$1 AND attempt=$2 AND device=$3 AND expires_at IS NULL",
    )
    .bind(job)
    .bind(attempt)
    .bind(device)
    .bind(sealed)
    .bind(expires_at)
    .bind(released_at)
    .execute(conn)
    .await?;
    if wrote.rows_affected() != 1 {
        return Err(Error::Unreadable);
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
pub(in crate::work) struct Revocation {
    pub job: JobId,
    pub attempt: i32,
    pub device: DeviceId,
    pub sealed: Vec<u8>,
}

pub(in crate::work) async fn due(
    db: &sqlx::PgPool,
    (now, retry_at): (DateTime<Utc>, DateTime<Utc>),
) -> Result<Vec<Revocation>, Error> {
    sqlx::query(
        "UPDATE jobs.repository_token_releases SET sealed=NULL WHERE sealed IS NOT NULL AND expires_at<=$1",
    ).bind(now).execute(db).await?;
    Ok(sqlx::query_as(
        "WITH due AS (
           SELECT r.job,r.attempt FROM jobs.repository_token_releases r
           JOIN jobs.jobs j ON j.id=r.job
           WHERE r.sealed IS NOT NULL AND r.expires_at>$1 AND (r.retry_at IS NULL OR r.retry_at<=$1)
             AND (r.released_at IS NULL OR j.attempt<>r.attempt OR j.device_id IS DISTINCT FROM r.device
                  OR j.stop_reason IS NOT NULL
                  OR NOT EXISTS (SELECT 1 FROM jobs.project_machines p
                                 WHERE p.project_id=j.project_id AND p.device_id=r.device))
           ORDER BY r.retry_at NULLS FIRST,r.expires_at,r.job,r.attempt
           LIMIT 16 FOR UPDATE OF r SKIP LOCKED
         )
         UPDATE jobs.repository_token_releases r SET retry_at=$2 FROM due
         WHERE r.job=due.job AND r.attempt=due.attempt
         RETURNING r.job,r.attempt,r.device,r.sealed",
    ).bind(now).bind(retry_at).fetch_all(db).await?)
}

pub(in crate::work) async fn revoked(db: &sqlx::PgPool, release: &Revocation) -> Result<(), Error> {
    sqlx::query(
        "UPDATE jobs.repository_token_releases SET sealed=NULL WHERE job=$1 AND attempt=$2",
    )
    .bind(release.job)
    .bind(release.attempt)
    .execute(db)
    .await?;
    Ok(())
}
