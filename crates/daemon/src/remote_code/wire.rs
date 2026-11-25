//! The K10 agent-thread claim, dispatch and evidence bodies.

use std::path::PathBuf;

use agent_threads::drive::{Mode, Program};
use chrono::{DateTime, Utc};
use daemon_core::wire::{Lease, Limits};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchParameters {
    pub launch: Uuid,
    pub account: Uuid,
    pub program: Program,
    pub folder: PathBuf,
    pub access: Mode,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum WorkType {
    AgentThread,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum WorkKind {
    Person,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClaimedLaunch {
    pub id: Uuid,
    #[serde(rename = "type")]
    work_type: WorkType,
    kind: WorkKind,
    pub attempt: u32,
    pub lease: Lease,
    limits: Limits,
    pub project: Uuid,
    pub parameters: LaunchParameters,
}

impl ClaimedLaunch {
    #[must_use]
    pub fn valid(&self) -> bool {
        self.work_type == WorkType::AgentThread
            && self.kind == WorkKind::Person
            && !self.id.is_nil()
            && !self.project.is_nil()
            && !self.parameters.launch.is_nil()
            && !self.parameters.account.is_nil()
            && self.attempt == 1
            && self.limits
                == (Limits {
                    seconds: 120,
                    idle_seconds: 0,
                    memory_bytes: 0,
                    disk_bytes: 0,
                    orphan_seconds: 0,
                })
    }
}

/// The device account comes from authenticated enrollment or claim context.
#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Owner {
    pub device: Uuid,
    pub account: Uuid,
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchRequest {
    pub device: Uuid,
    pub launch: Uuid,
    pub work: Uuid,
    pub attempt: u32,
    pub runner: Uuid,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Granted {
    pub(crate) authorization: Uuid,
    pub(crate) valid_until: DateTime<Utc>,
    pub(crate) remaining_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotCreated {
    Refused,
    Failed,
    Stopped,
    Expired,
}

#[derive(Serialize)]
pub struct Evidence {
    pub device: Uuid,
    pub launch: Uuid,
    pub work: Uuid,
    pub attempt: u32,
    #[serde(flatten)]
    pub outcome: EvidenceOutcome,
}

#[derive(Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum EvidenceOutcome {
    Created {
        thread: Uuid,
        cause: (),
        #[serde(skip_serializing_if = "Option::is_none")]
        publication: Option<Publication>,
    },
    NotCreated {
        cause: NotCreated,
    },
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Publication {
    Withdrawn,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchReceipt {
    pub launch: Uuid,
    pub state: String,
    pub cause: Option<String>,
    pub thread: Option<Uuid>,
    pub binding: String,
    pub first_command: Uuid,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Commands {
    pub(crate) commands: Vec<super::command::Delivered>,
}

#[derive(Serialize)]
pub struct ControlUpload<'a> {
    pub device: Uuid,
    pub launch: Uuid,
    pub runner: Uuid,
    pub offer: &'a super::command::Offer,
}
