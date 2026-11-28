//! I/O only (CQ-032): writes take the caller's connection, so a change commits with its audit entry or event.

pub(super) mod application_files;
pub(super) mod checkout;
pub(super) mod hosting;
mod job_row;
mod logs;
mod machines;
mod placements;
pub(super) mod source_placement;

use chrono::{DateTime, Utc};
use daemon_core::model_work::names::Word as _;
use daemon_core::wire::WorkType;
use sqlx::PgConnection;

use job_row::{JobRow, SELECT_JOB};
pub(super) use logs::{append_log, log_of};
pub(super) use machines::{add_fault, machine_locked, machines, save_machine, set_off_until};
pub(super) use placements::{
    decisions_of, offer_of, others_asking, pin, pins, save_decisions, save_said, set_excluded,
    unpin,
};

use super::{
    Error,
    model::{Job, Target},
};
use crate::ids::{DeviceId, JobId};

const CLAIM_LOOKS_AT: i64 = 50;

pub(super) fn word<T: serde::Serialize>(value: &T) -> Result<String, Error> {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(text)) => Ok(text),
        _ => Err(Error::Unreadable),
    }
}

pub(super) fn read_word<T: serde::de::DeserializeOwned>(text: &str) -> Result<T, Error> {
    serde_json::from_value(serde_json::Value::String(text.to_owned()))
        .map_err(|_| Error::Unreadable)
}

pub(super) fn count(value: i64) -> Result<u64, Error> {
    u64::try_from(value).map_err(|_| Error::Unreadable)
}

pub(super) fn stored(value: u64) -> Result<i64, Error> {
    i64::try_from(value).map_err(|_| Error::Unreadable)
}

async fn jobs_where(
    conn: &mut PgConnection,
    condition: &str,
    at: Option<DateTime<Utc>>,
    device: Option<DeviceId>,
) -> Result<Vec<Job>, Error> {
    let sql = format!("{SELECT_JOB}{condition}");
    let rows: Vec<JobRow> = sqlx::query_as(&sql)
        .bind(at)
        .bind(device)
        .fetch_all(conn)
        .await?;
    rows.into_iter().map(Job::try_from).collect()
}

/// The jobs with these ids, unlocked.
pub(super) async fn jobs_of(conn: &mut PgConnection, ids: &[JobId]) -> Result<Vec<Job>, Error> {
    let sql = format!("{SELECT_JOB}WHERE id = ANY($1)");
    let rows: Vec<JobRow> = sqlx::query_as(&sql).bind(ids).fetch_all(conn).await?;
    rows.into_iter().map(Job::try_from).collect()
}

/// The newest job of `work_type` that each machine ended `done`.
pub(super) async fn newest_done(
    conn: &mut PgConnection,
    work_type: WorkType,
) -> Result<Vec<Job>, Error> {
    let sql = format!(
        "{SELECT_JOB}WHERE id IN (SELECT DISTINCT ON (machine_id) id FROM jobs.jobs \
         WHERE work_type = $1 AND state = 'done' AND machine_id IS NOT NULL \
         ORDER BY machine_id, ended_at DESC NULLS LAST)"
    );
    let rows: Vec<JobRow> = sqlx::query_as(&sql)
        .bind(word(&work_type)?)
        .fetch_all(conn)
        .await?;
    rows.into_iter().map(Job::try_from).collect()
}

pub(super) async fn source_job(db: &sqlx::PgPool, id: JobId) -> Result<Option<Job>, Error> {
    let sql = format!(
        "{SELECT_JOB}WHERE id = $1 AND work_type = 'build' AND parameters->>'recipe' = 'source_image_v1'"
    );
    let row: Option<JobRow> = sqlx::query_as(&sql).bind(id).fetch_optional(db).await?;
    row.map(Job::try_from).transpose()
}

pub(super) async fn job_locked(conn: &mut PgConnection, id: JobId) -> Result<Option<Job>, Error> {
    let sql = format!("{SELECT_JOB}WHERE id = $1 FOR UPDATE");
    let row: Option<JobRow> = sqlx::query_as(&sql).bind(id).fetch_optional(conn).await?;
    row.map(Job::try_from).transpose()
}

/// A job that a different transaction has locked is skipped, so two claims never take one job (BLD-004).
pub(super) async fn oldest_queued(conn: &mut PgConnection) -> Result<Vec<Job>, Error> {
    let condition = format!(
        "WHERE state = 'queued' AND kind <> 'host' AND $1::timestamptz IS NULL AND $2::uuid IS NULL \
         ORDER BY created_at, id LIMIT {CLAIM_LOOKS_AT} FOR UPDATE SKIP LOCKED"
    );
    jobs_where(conn, &condition, None, None).await
}

pub(super) async fn held_by(conn: &mut PgConnection, device: DeviceId) -> Result<Vec<Job>, Error> {
    let condition = "WHERE device_id = $2 AND $1::timestamptz IS NULL ORDER BY id FOR UPDATE";
    jobs_where(conn, condition, None, Some(device)).await
}

pub(super) async fn ended_leases(
    conn: &mut PgConnection,
    now: DateTime<Utc>,
) -> Result<Vec<Job>, Error> {
    let condition = "WHERE lease_ends_at <= $1 AND kind <> 'host' AND $2::uuid IS NULL \
                     ORDER BY lease_ends_at, id FOR UPDATE SKIP LOCKED";
    jobs_where(conn, condition, Some(now), None).await
}

/// The 5 minutes past the time limit come from SF-011.
pub(super) async fn over_time(
    conn: &mut PgConnection,
    now: DateTime<Utc>,
) -> Result<Vec<Job>, Error> {
    let condition = "WHERE lease_ends_at IS NOT NULL AND $2::uuid IS NULL \
                     AND claimed_at + make_interval(secs => limit_seconds + 300) <= $1 \
                     ORDER BY claimed_at, id FOR UPDATE SKIP LOCKED";
    jobs_where(conn, condition, Some(now), None).await
}

pub(super) async fn insert(
    conn: &mut PgConnection,
    job: &Job,
    host_api_key: Option<&str>,
) -> Result<(), Error> {
    let asked = &job.asked;
    let (repository, actor) = match &asked.target {
        Target::Repository(repository) => (Some(repository), None),
        Target::Model { actor } => (None, Some(actor.as_str())),
        Target::Host => (None, None),
    };
    sqlx::query(
        "INSERT INTO jobs.jobs (id, work_type, kind, os, architecture, project_id, forge, owner, \
         name, parameters, secrets, limit_seconds, limit_idle_seconds, limit_memory_bytes, \
         limit_disk_bytes, limit_orphan_seconds, state, created_at, state_changed_at, machine_id, \
         actor, host_api_key) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, \
         $18, $18, $19, $20, $21)",
    )
    .bind(job.id)
    .bind(word(&asked.work_type)?)
    .bind(word(&asked.kind)?)
    .bind(asked.os.as_ref().map(word).transpose()?)
    .bind(asked.architecture.as_ref().map(word).transpose()?)
    .bind(asked.project)
    .bind(repository.map(|each| &each.forge))
    .bind(repository.map(|each| &each.owner))
    .bind(repository.map(|each| &each.name))
    .bind(&asked.parameters)
    .bind(&asked.secrets)
    .bind(stored(asked.limits.seconds)?)
    .bind(stored(asked.limits.idle_seconds)?)
    .bind(stored(asked.limits.memory_bytes)?)
    .bind(stored(asked.limits.disk_bytes)?)
    .bind(stored(asked.limits.orphan_seconds)?)
    .bind(job.state.as_str())
    .bind(job.created_at)
    .bind(asked.machine)
    .bind(actor)
    .bind(host_api_key)
    .execute(conn)
    .await?;
    Ok(())
}

pub(super) async fn save(conn: &mut PgConnection, job: &Job) -> Result<(), Error> {
    let result = job
        .result
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|_| Error::Unreadable)?;
    sqlx::query(
        "UPDATE jobs.jobs SET state = $2, attempt = $3, lost = $4, device_id = $5, \
         last_device_id = $6, refused_by = $7, lease_ends_at = $8, claimed_at = $9, \
         stop_reason = $10, reason = $11, result = $12, log_end = $13, state_changed_at = $14, \
         ended_at = $15, progress = $16 WHERE id = $1",
    )
    .bind(job.id)
    .bind(job.state.as_str())
    .bind(i32::try_from(job.attempt).map_err(|_| Error::Unreadable)?)
    .bind(i32::from(job.lost))
    .bind(job.holder)
    .bind(job.last_device)
    .bind(&job.refused_by)
    .bind(job.lease_ends_at)
    .bind(job.claimed_at)
    .bind(job.stop.as_ref().map(word).transpose()?)
    .bind(&job.reason)
    .bind(result)
    .bind(stored(job.log_end)?)
    .bind(job.changed_at)
    .bind(job.ended_at)
    .bind(job.progress.map(|progress| serde_json::json!(progress)))
    .execute(&mut *conn)
    .await?;
    if job.asked.work_type == WorkType::ApplicationApply && job.state.is_final() {
        application_files::clear(conn, job.id).await?;
    }
    Ok(())
}
