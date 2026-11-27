//! Keep released and refused tokens available solely for durable revocation.
use chrono::{DateTime, TimeDelta, Utc};
use futures_util::{StreamExt as _, stream};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    engine::sources::{Scheduling, revoke_repository_checkout},
    fleet::credentials::Sealed,
    ids::DeviceId,
    work::jobs::Error,
};

pub(super) fn context(request: Uuid, device: DeviceId) -> String {
    format!("workspace-checkout:{request}:{device}")
}

pub(super) async fn reserve(
    conn: &mut PgConnection,
    request: Uuid,
    device: DeviceId,
    now: DateTime<Utc>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO agent_threads.workspace_checkout_releases(request,device,reserved_at)
         VALUES ($1,$2,$3) ON CONFLICT DO NOTHING",
    )
    .bind(request)
    .bind(device)
    .bind(now)
    .execute(conn)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub(super) async fn retain(
    pool: &sqlx::PgPool,
    (request, device): (Uuid, DeviceId),
    (sealed, minted_until, decision_until): (&[u8], DateTime<Utc>, DateTime<Utc>),
) -> Result<(), Error> {
    let result = sqlx::query(
        "UPDATE agent_threads.workspace_checkout_releases SET sealed=$3,minted_until=$4,decision_until=$5
         WHERE request=$1 AND device=$2 AND minted_until IS NULL",
    )
    .bind(request).bind(device).bind(sealed).bind(minted_until).bind(decision_until).execute(pool).await?;
    if result.rows_affected() != 1 {
        return Err(Error::Unreadable);
    }
    Ok(())
}

pub(super) async fn decide(
    conn: &mut PgConnection,
    (request, device): (Uuid, DeviceId),
    (released_until, now): (Option<DateTime<Utc>>, DateTime<Utc>),
) -> Result<(), Error> {
    let changed = sqlx::query(
        "UPDATE agent_threads.workspace_checkout_releases SET release_until=$3,decision_until=NULL
         WHERE request=$1 AND device=$2 AND decision_until IS NOT NULL
           AND ($3::timestamptz IS NULL OR (decision_until>$4 AND sealed IS NOT NULL AND retry_at IS NULL))",
    )
    .bind(request).bind(device).bind(released_until).bind(now).execute(conn).await?;
    if released_until.is_some() && changed.rows_affected() != 1 {
        return Err(Error::Unreadable);
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct Revocation {
    request: Uuid,
    device: DeviceId,
    sealed: Vec<u8>,
}

pub(in crate::work::checkout) async fn revoke_due(sources: &Scheduling) -> Result<(), Error> {
    let now = sources.clock.now();
    sqlx::query("UPDATE agent_threads.workspace_checkout_releases SET sealed=NULL WHERE sealed IS NOT NULL AND minted_until<=$1")
        .bind(now).execute(&sources.pool).await?;
    sqlx::query(
        "DELETE FROM agent_threads.workspace_checkout_releases x WHERE sealed IS NULL AND reserved_at<$1
         AND NOT EXISTS(SELECT 1 FROM agent_threads.workspace_requests r WHERE r.request=x.request)",
    ).bind(now-TimeDelta::hours(2)).execute(&sources.pool).await?;
    let due: Vec<Revocation> = sqlx::query_as(
        "WITH due AS (
           SELECT x.request FROM agent_threads.workspace_checkout_releases x
           LEFT JOIN agent_threads.workspace_requests r USING (request)
           WHERE x.sealed IS NOT NULL AND x.minted_until>$1 AND (x.retry_at IS NULL OR x.retry_at<=$1)
             AND ((x.release_until IS NULL AND (x.decision_until IS NULL OR x.decision_until<=$1))
                  OR x.release_until<=$1 OR r.state IS DISTINCT FROM 'offered'
                  OR NOT EXISTS(SELECT 1 FROM metrics.daemons d WHERE d.device_id=x.device AND d.expires_at>$1))
           ORDER BY x.retry_at NULLS FIRST,x.minted_until,x.request LIMIT 16 FOR UPDATE OF x SKIP LOCKED
         )
         UPDATE agent_threads.workspace_checkout_releases x SET retry_at=$2 FROM due
         WHERE x.request=due.request RETURNING x.request,x.device,x.sealed",
    ).bind(now).bind(now+TimeDelta::minutes(1)).fetch_all(&sources.pool).await?;
    stream::iter(due)
        .for_each_concurrent(4, |row| async move {
            if revoke(sources, row).await.is_err() {
                tracing::warn!("workspace checkout credential revocation will retry");
            }
        })
        .await;
    Ok(())
}

async fn revoke(sources: &Scheduling, release: Revocation) -> Result<(), Error> {
    let context = context(release.request, release.device);
    let sealed = Sealed::from_bytes(&release.sealed).map_err(|_| Error::Unreadable)?;
    let value = sources
        .vault
        .open(&sealed, context.as_bytes())
        .map_err(|_| Error::Unreadable)?;
    revoke_repository_checkout(&sources.transport, &value)
        .await
        .map_err(|_| Error::Unreadable)?;
    sqlx::query(
        "UPDATE agent_threads.workspace_checkout_releases SET sealed=NULL,decision_until=NULL WHERE request=$1",
    )
    .bind(release.request)
    .execute(&sources.pool)
    .await?;
    Ok(())
}
