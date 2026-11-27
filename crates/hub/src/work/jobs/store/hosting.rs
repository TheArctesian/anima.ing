//! Host queue reads and admission fences on the named device.

use daemon_core::wire::{Claim, WorkType};

use chrono::{DateTime, TimeDelta, Utc};
use sqlx::{PgConnection, PgPool};
use std::collections::HashSet;
use uuid::Uuid;

use super::{JobRow, SELECT_JOB};
use crate::{
    ids::{DeviceId, JobId},
    work::jobs::{Error, Job},
};

pub(in crate::work::jobs) const WAIT: TimeDelta = TimeDelta::minutes(10);

pub(in crate::work::jobs) async fn duplicate(
    conn: &mut PgConnection,
    device: DeviceId,
    (job, now): (&Job, DateTime<Utc>),
) -> Result<bool, Error> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (
             SELECT 1 FROM jobs.jobs WHERE kind = 'host' AND machine_id = $1
              AND state = 'queued' AND work_type = $2 AND parameters = $3
              AND created_at > $4
         )",
    )
    .bind(device)
    .bind(job.asked.work_type.as_str())
    .bind(&job.asked.parameters)
    .bind(now - WAIT)
    .fetch_one(conn)
    .await?)
}

pub(in crate::work::jobs) async fn key_has_room(
    conn: &mut PgConnection,
    device: DeviceId,
    key: &str,
    now: DateTime<Utc>,
) -> Result<bool, Error> {
    let (recent, open): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE created_at > $3),
                count(*) FILTER (WHERE state NOT IN ('done', 'failed', 'stopped'))
         FROM jobs.jobs WHERE machine_id = $1 AND host_api_key = $2
           AND (created_at > $3 OR state NOT IN ('done', 'failed', 'stopped'))",
    )
    .bind(device)
    .bind(key)
    .bind(now - TimeDelta::minutes(1))
    .fetch_one(conn)
    .await?;
    Ok(recent < 6 && open < 12)
}

pub(in crate::work::jobs) async fn next(
    conn: &mut PgConnection,
    device: DeviceId,
    (claim, allow_mutation, now): (&Claim, bool, DateTime<Utc>),
) -> Result<Option<Job>, Error> {
    let sql = format!(
        "{SELECT_JOB}queued WHERE kind = 'host' AND state = 'queued' AND machine_id = $1
         AND created_at > $2 AND ($3 OR (parameters->>'action' = 'stop') IS TRUE)
         AND work_type = ANY($4)
         AND (work_type <> 'application_apply' OR NOT EXISTS (
            SELECT 1 FROM jobs.jobs active WHERE active.machine_id = $1
               AND active.work_type = 'application_apply'
               AND active.parameters->>'application' = queued.parameters->>'application'
               AND active.state IN ('claimed', 'preparing', 'running', 'finishing')
         ))
         ORDER BY (parameters->>'action' = 'stop') IS TRUE DESC, (host_api_key IS NOT NULL),
           created_at, id LIMIT 1 FOR UPDATE SKIP LOCKED"
    );
    let types: Vec<_> = claim.types.iter().map(|kind| kind.as_str()).collect();
    let row: Option<JobRow> = sqlx::query_as(&sql)
        .bind(device)
        .bind(now - WAIT)
        .bind(allow_mutation)
        .bind(&types)
        .fetch_optional(conn)
        .await?;
    row.map(Job::try_from).transpose()
}

pub(in crate::work::jobs) async fn unclaimed_expired(
    conn: &mut PgConnection,
    now: DateTime<Utc>,
) -> Result<Vec<Job>, Error> {
    let sql = format!(
        "{SELECT_JOB}WHERE kind = 'host' AND state = 'queued' AND created_at <= $1
         ORDER BY created_at, id LIMIT 50 FOR UPDATE SKIP LOCKED"
    );
    let rows: Vec<JobRow> = sqlx::query_as(&sql)
        .bind(now - WAIT)
        .fetch_all(conn)
        .await?;
    rows.into_iter().map(Job::try_from).collect()
}

pub(in crate::work::jobs) async fn request(
    db: &PgPool,
    id: JobId,
    kind: WorkType,
) -> Result<Option<Job>, Error> {
    let sql = format!("{SELECT_JOB}WHERE id = $1 AND kind = 'host' AND work_type = $2");
    let row: Option<JobRow> = sqlx::query_as(&sql)
        .bind(id)
        .bind(kind.as_str())
        .fetch_optional(db)
        .await?;
    row.map(Job::try_from).transpose()
}

pub(in crate::work::jobs) async fn installed_applications(
    db: &PgPool,
    device: DeviceId,
    candidates: &[Uuid],
) -> Result<HashSet<Uuid>, Error> {
    let applications: Vec<Uuid> = sqlx::query_scalar(
        "SELECT app.application_id FROM jobs.application_machines app
         WHERE app.device_id = $1 AND app.application_id = ANY($2)
           AND EXISTS (
               SELECT 1 FROM jobs.jobs applied WHERE applied.machine_id = app.device_id
                 AND applied.kind = 'host' AND applied.work_type = 'application_apply'
                 AND applied.state = 'done'
                 AND applied.parameters->>'application' = app.application_id::text
           )",
    )
    .bind(device)
    .bind(candidates)
    .fetch_all(db)
    .await?;
    Ok(applications.into_iter().collect())
}

pub(in crate::work::jobs) async fn apply_targets(
    db: &PgPool,
    fresh: DateTime<Utc>,
) -> Result<Vec<(DeviceId, String)>, Error> {
    Ok(sqlx::query_as(
        "SELECT device.id, device.name FROM jobs.machines machine
         JOIN devices.devices device ON device.id = machine.device_id
         WHERE machine.last_claim_at >= $1 AND 'application_apply' = ANY(machine.types)
           AND NOT machine.off_by_operator
         ORDER BY device.name, device.id",
    )
    .bind(fresh)
    .fetch_all(db)
    .await?)
}

pub(in crate::work::jobs) async fn available(
    db: &PgPool,
    device: DeviceId,
    fresh: DateTime<Utc>,
) -> Result<bool, Error> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM jobs.machines
         WHERE device_id = $1 AND last_claim_at >= $2 AND 'container_action' = ANY(types))",
    )
    .bind(device)
    .bind(fresh)
    .fetch_one(db)
    .await?)
}
