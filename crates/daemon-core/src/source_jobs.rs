//! Closed local source-job protocol between the enrolled daemon and the root owner.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const SOCKET: &str = "/run/anima-source-jobs/control.sock";
pub const MAX_REQUEST_BYTES: usize = 65_536;
pub const MAX_REPLY_BYTES: usize = 262_144;
pub const MAX_CHECKOUT_TOKEN_BYTES: usize = 16 * 1024;
pub const CHECKOUT_READY: &[u8; 6] = b"ready\n";

/// This recipe accepts one checkout credential for a project-bound item.
#[must_use]
pub fn private_checkout(job: &crate::wire::WorkJob) -> bool {
    job.project.is_some() && job.secrets.as_slice() == [crate::wire::REPOSITORY_TOKEN]
}

#[must_use]
pub fn valid_checkout_token(value: &[u8]) -> bool {
    !value.is_empty()
        && value.len() <= MAX_CHECKOUT_TOKEN_BYTES
        && value.iter().all(u8::is_ascii_graphic)
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub job: Uuid,
    pub attempt: u32,
    pub until: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub execution: Uuid,
    pub job: Uuid,
    pub attempt: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub device: Uuid,
    pub origin: String,
    pub action: Action,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Action {
    Export {
        execution: Uuid,
        archive_digest: String,
        manifest_digest: String,
    },
    Release {
        execution: Uuid,
        receipt: Option<crate::artifact::Receipt>,
    },
    Retire {
        identity: Attempt,
    },
    Capacity,
    List,
    Register {
        execution: Uuid,
        item: Value,
    },
    Start {
        execution: Uuid,
    },
    Status {
        execution: Uuid,
    },
    Renew {
        execution: Uuid,
        lease: Grant,
    },
    Cancel {
        execution: Uuid,
    },
    /// When an expiry is present, the value follows the owner's ready signal as raw bytes.
    Checkout {
        identity: Attempt,
        expires_at: Option<DateTime<Utc>>,
    },
}
