//! I/O only (CQ-032): writes take the caller's connection, so a change commits with its audit entry or event.

pub(super) mod application_files;
pub(super) mod checkout;
pub(super) mod hosting;
mod logs;
mod machines;
mod placements;
pub(super) mod source_placement;

use chrono::{DateTime, Utc};
use daemon_core::model_work::names::{Actor, Word as _};
use daemon_core::wire::{
    Limits, StopReason, WorkArchitecture, WorkKind, WorkOs, WorkRepository, WorkType,
};
use sqlx::PgConnection;

pub(super) use logs::{append_log, log_of};
pub(super) use machines::{add_fault, machine_locked, machines, save_machine, set_off_until};
pub(super) use placements::{
    decisions_of, offer_of, others_asking, pin, pins, save_decisions, save_said, set_excluded,
    unpin,
};

use super::{
    Error,
    model::{Job, NewJob, State, Target},
};
use crate::ids::{DeviceId, JobId, ProjectId};

const CLAIM_LOOKS_AT: i64 = 50;

const SELECT_JOB: &str = "SELECT id, work_type, kind, os, architecture, project_id, forge, owner, \
     name, parameters, secrets, limit_seconds, limit_idle_seconds, limit_memory_bytes, \
     limit_disk_bytes, limit_orphan_seconds, state, attempt, lost, device_id, last_device_id, \
     refused_by, lease_ends_at, claimed_at, stop_reason, reason, result, log_end, created_at, \
     state_changed_at, ended_at, machine_id, actor, progress FROM jobs.jobs ";

#[derive(sqlx::FromRow)]
struct JobRow {
    id: JobId,
    work_type: String,
    kind: String,
    os: Option<String>,
    architecture: Option<String>,
    project_id: Option<ProjectId>,
    forge: Option<String>,
    owner: Option<String>,
    name: Option<String>,
    parameters: serde_json::Value,
    secrets: Vec<String>,
    limit_seconds: i64,
    limit_idle_seconds: i64,
    limit_memory_bytes: i64,
    limit_disk_bytes: i64,
    limit_orphan_seconds: i64,
    state: String,
    attempt: i32,
    lost: i32,
    device_id: Option<DeviceId>,
    last_device_id: Option<DeviceId>,
    refused_by: Vec<DeviceId>,
    lease_ends_at: Option<DateTime<Utc>>,
    claimed_at: Option<DateTime<Utc>>,
    stop_reason: Option<String>,
    reason: Option<String>,
    result: Option<serde_json::Value>,
    log_end: i64,
    created_at: DateTime<Utc>,
    state_changed_at: DateTime<Utc>,
    ended_at: Option<DateTime<Utc>>,
    machine_id: Option<DeviceId>,
    actor: Option<String>,
    progress: Option<serde_json::Value>,
}

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

impl JobRow {
    fn asked(&self) -> Result<NewJob, Error> {
        Ok(NewJob {
            work_type: read_word::<WorkType>(&self.work_type)?,
            kind: read_word::<WorkKind>(&self.kind)?,
            os: self.os.as_deref().map(read_word::<WorkOs>).transpose()?,
            architecture: self
                .architecture
                .as_deref()
                .map(read_word::<WorkArchitecture>)
                .transpose()?,
            project: self.project_id,
            target: self.target()?,
            parameters: self.parameters.clone(),
            secrets: self.secrets.clone(),
            limits: Limits {
                seconds: count(self.limit_seconds)?,
                idle_seconds: count(self.limit_idle_seconds)?,
                memory_bytes: count(self.limit_memory_bytes)?,
                disk_bytes: count(self.limit_disk_bytes)?,
                orphan_seconds: count(self.limit_orphan_seconds)?,
            },
            machine: self.machine_id,
        })
    }
}

impl JobRow {
    fn target(&self) -> Result<Target, Error> {
        match (&self.actor, &self.forge, &self.owner, &self.name) {
            (Some(actor), None, None, None) => Ok(Target::Model {
                actor: Actor::parse(actor).ok_or(Error::Unreadable)?,
            }),
            (None, Some(forge), Some(owner), Some(name)) => {
                Ok(Target::Repository(WorkRepository {
                    forge: forge.clone(),
                    owner: owner.clone(),
                    name: name.clone(),
                }))
            }
            (None, None, None, None) if self.kind == "host" => Ok(Target::Host),
            _ => Err(Error::Unreadable),
        }
    }
}

impl TryFrom<JobRow> for Job {
    type Error = Error;

    fn try_from(row: JobRow) -> Result<Self, Error> {
        Ok(Self {
            id: row.id,
            asked: row.asked()?,
            state: State::from_stored(&row.state).ok_or(Error::Unreadable)?,
            attempt: u32::try_from(row.attempt).map_err(|_| Error::Unreadable)?,
            lost: u8::try_from(row.lost).map_err(|_| Error::Unreadable)?,
            holder: row.device_id,
            last_device: row.last_device_id,
            refused_by: row.refused_by,
            lease_ends_at: row.lease_ends_at,
            claimed_at: row.claimed_at,
            stop: row
                .stop_reason
                .as_deref()
                .map(read_word::<StopReason>)
                .transpose()?,
            reason: row.reason,
            result: row
                .result
                .map(serde_json::from_value)
                .transpose()
                .map_err(|_| Error::Unreadable)?,
            log_end: count(row.log_end)?,
            progress: row
                .progress
                .map(serde_json::from_value)
                .transpose()
                .map_err(|_| Error::Unreadable)?,
            created_at: row.created_at,
            changed_at: row.state_changed_at,
            ended_at: row.ended_at,
        })
    }
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

/// The newest job of `work_type` that each machine ended `done`; of two that ended in the same instant, the one made
/// later (its UUID version 7 is larger).
pub(super) async fn newest_done(
    conn: &mut PgConnection,
    work_type: WorkType,
) -> Result<Vec<Job>, Error> {
    let sql = format!(
        "{SELECT_JOB}WHERE id IN (SELECT DISTINCT ON (machine_id) id FROM jobs.jobs \
         WHERE work_type = $1 AND state = 'done' AND machine_id IS NOT NULL \
         ORDER BY machine_id, ended_at DESC NULLS LAST, id DESC)"
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
