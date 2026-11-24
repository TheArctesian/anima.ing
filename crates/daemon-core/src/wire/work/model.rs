//! The `model` member of a work result and the progress of a pull (K11 Sections 3 and 8).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::WorkResult;

impl WorkResult {
    /// The copy that the item changed; `None` for the state of `model_report`.
    #[must_use]
    pub fn copy(&self) -> Option<&CopyOutcome> {
        match &self.model {
            Some(ModelOutcome::Copy(copy)) => Some(copy),
            _ => None,
        }
    }

    /// The found models of a `model_report` (K11 Section 8); `None` for each other result.
    #[must_use]
    pub fn found(&self) -> Option<&[FoundEntry]> {
        match &self.model {
            Some(ModelOutcome::State(state)) => Some(&state.found),
            _ => None,
        }
    }

    /// The copies that each server holds, of a `model_report` (K11 Section 11.1); `None` for each other result.
    #[must_use]
    pub fn servers(&self) -> Option<&[ServerLoads]> {
        match &self.model {
            Some(ModelOutcome::State(state)) => Some(&state.servers),
            _ => None,
        }
    }
}

/// The `model` member of a result: a copy outcome (K11 Section 3), or the model state that
/// `model_report` gives (Section 8), of which the hub keeps only the time and `found`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ModelOutcome {
    Copy(CopyOutcome),
    State(ModelState),
}

/// The model state of `model_report` as the hub keeps it (K11 Section 8): the time, `found`, and the
/// copies that each server holds (Section 11.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelState {
    pub at: String,
    #[serde(default)]
    pub found: Vec<FoundEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub servers: Vec<ServerLoads>,
}

/// The copies that one server holds in memory (K11 Section 11.1, `servers[].models`); the hub keeps
/// no other member of a server here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerLoads {
    pub server: Uuid,
    #[serde(default)]
    pub models: Vec<LoadEntry>,
}

/// One copy that a server holds (K11 Section 11.1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadEntry {
    pub copy: Uuid,
    /// `loading`, `ready`, `active` or `unloading`.
    pub state: String,
    pub reserved_bytes: u64,
    pub idle_seconds: u32,
    pub pinned: bool,
    pub used_seconds_ago: u64,
}

/// A model that a different program keeps on the machine (K11 Sections 2.3 and 8): its key,
/// never a path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundEntry {
    pub found: String,
    pub tool: String,
    pub name: String,
    pub format: String,
    pub files: u32,
    pub bytes: u64,
    pub usable: bool,
}

/// The size and the SHA-256 of a file that `copy_adopt` read (K11 Section 3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHash {
    pub size_bytes: u64,
    pub sha256: String,
}

/// The `model` member of the result of a model item (K11 Section 3): the copy, its state of
/// K11 Section 11, its bytes and its file count. Codes and numbers only.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopyOutcome {
    pub copy: Uuid,
    pub copy_state: String,
    pub bytes: u64,
    pub files: u32,
    /// Only `copy_adopt`: what the daemon read of each file (K11 Section 3).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub file_hashes: Vec<FileHash>,
}

/// The progress of a running `model_pull` (K11 Section 3, PULL-007). The hub computes the
/// time to completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub bytes: u64,
    pub total_bytes: u64,
    pub bytes_per_second: u64,
}
