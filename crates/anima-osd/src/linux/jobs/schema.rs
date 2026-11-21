use super::super::probe::Diagnostics;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize, Serialize)]
pub struct Request {
    pub version: u8,
    #[serde(flatten)]
    pub operation: Operation,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    #[serde(rename = "operation_start")]
    Start { request_id: Uuid, action: Action },
    #[serde(rename = "operation_get")]
    Get { request_id: Uuid },
    #[serde(rename = "operation_list")]
    List { after: Option<Uuid> },
    #[serde(rename = "operation_cancel")]
    Cancel { operation_id: Uuid },
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Diagnostics,
}

#[derive(Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Queued,
    Running,
    Succeeded,
    Cancelled,
    Interrupted,
}

impl State {
    pub fn terminal(self) -> bool {
        !matches!(self, Self::Queued | Self::Running)
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub id: Uuid,
    pub request_id: Uuid,
    pub action: Action,
    pub state: State,
    pub revision: u64,
    pub cancel_requested: bool,
    pub created_seconds: u64,
    pub updated_seconds: u64,
    pub boot_id: String,
    pub result: Option<Diagnostics>,
}

#[derive(Serialize, Deserialize)]
pub struct Reply {
    pub version: u8,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "result", content = "value", rename_all = "snake_case")]
pub enum Outcome {
    Operation(Record),
    Operations {
        records: Vec<Record>,
        next: Option<Uuid>,
    },
    Error(Failure),
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    #[error("invalid_request")]
    InvalidRequest,
    #[error("jobs_unavailable")]
    JobsUnavailable,
    #[error("timed_out")]
    TimedOut,
    #[error("busy")]
    Busy,
    #[error("not_found")]
    NotFound,
    #[error("cancel_not_allowed")]
    CancelNotAllowed,
    #[error("journal_unavailable")]
    JournalUnavailable,
}

impl Reply {
    pub fn new(result: Result<Outcome, Failure>) -> Self {
        Self {
            version: 2,
            outcome: result.unwrap_or_else(Outcome::Error),
        }
    }
}
