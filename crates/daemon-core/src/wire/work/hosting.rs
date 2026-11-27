//! Container work on one machine, outside the daemon's job isolation.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Lease, Limits, WorkKind, WorkType};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContainerVerb {
    Start,
    Stop,
    Restart,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContainerAction {
    pub container: String,
    pub action: ContainerVerb,
}

impl ContainerAction {
    /// K10 takes the full Docker ID, in canonical hexadecimal form.
    #[must_use]
    pub fn full_id(&self) -> bool {
        self.container.len() == 64
            && self
                .container
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    }
}

pub const COMPOSE_FILE: &str = "compose_file";
pub const ENVIRONMENT_FILE: &str = "environment_file";

/// K10 bound inputs of one managed Compose deployment.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplicationApply {
    pub application: Uuid,
    pub file_sha256: String,
    pub environment_sha256: Option<String>,
    pub pull: bool,
    pub keep: u8,
}

/// The observed result of the managed application's health phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplicationHealth {
    Healthy,
    Unhealthy,
    NoCheck,
    NotWaited,
    #[serde(other)]
    Unknown,
}

/// A host item has no repository member, even when serialized by the same work route as jobs.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HostWork {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub work_type: WorkType,
    pub kind: WorkKind,
    pub attempt: u32,
    pub lease: Lease,
    pub limits: Limits,
    pub project: Option<Uuid>,
    pub secrets: Vec<String>,
    pub parameters: serde_json::Value,
}
