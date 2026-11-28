//! A row of `jobs.jobs` and the job that it holds.

use chrono::{DateTime, Utc};
use daemon_core::model_work::names::{Actor, Word as _};
use daemon_core::wire::{
    Limits, StopReason, WorkArchitecture, WorkKind, WorkOs, WorkRepository, WorkType,
};

use super::{count, read_word};
use crate::{
    ids::{DeviceId, JobId, ProjectId},
    work::jobs::{
        Error,
        model::{Job, NewJob, State, Target},
    },
};

pub(super) const SELECT_JOB: &str = "SELECT id, work_type, kind, os, architecture, project_id, forge, owner, \
     name, parameters, secrets, limit_seconds, limit_idle_seconds, limit_memory_bytes, \
     limit_disk_bytes, limit_orphan_seconds, state, attempt, lost, device_id, last_device_id, \
     refused_by, lease_ends_at, claimed_at, stop_reason, reason, result, log_end, created_at, \
     state_changed_at, ended_at, machine_id, actor, progress FROM jobs.jobs ";

#[derive(sqlx::FromRow)]
pub(super) struct JobRow {
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
