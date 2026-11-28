//! The local runner's peer-bound Git summaries (TC7 W3).

use serde::{Deserialize, Serialize};

use super::Scope;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub repository: String,
    pub read: Read,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum Read {
    Status,
    Checkpoints { limit: u32, after: Option<u32> },
    Changes { scope: Scope },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Refusal {
    Unsupported,
    Unbound,
    NoWorktree,
    UnknownCheckpoint,
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Status {
        status: Status,
    },
    Checkpoints {
        checkpoints: Vec<Checkpoint>,
        next_cursor: Option<u32>,
    },
    Changes {
        changes: Changes,
    },
    Refused {
        why: Refusal,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the summary exposes four branch and remote flags without names or URLs"
)]
pub struct Status {
    pub detached: bool,
    pub is_default_branch: bool,
    pub has_remote: bool,
    pub has_upstream: bool,
    pub ahead: u32,
    pub behind: u32,
    pub ahead_of_default: u32,
    pub files: usize,
    pub additions: u32,
    pub deletions: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkpoint {
    pub turn: u32,
    pub at: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Changes {
    pub scope: Scope,
    pub files: usize,
    pub added: usize,
    pub modified: usize,
    pub deleted: usize,
    pub renamed: usize,
    pub binary: usize,
    pub additions: u32,
    pub deletions: u32,
}
