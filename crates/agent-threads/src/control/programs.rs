//! What the programs of this computer last said of themselves: their accounts (THR-132) and their
//! models (TH104) and the state of each list (TH117). In the memory of the runner only.

use serde::{Deserialize, Serialize};

use crate::drive::{AccountFacts, ModelChoice, Program};

/// The account of one program of this computer, as its newest report said it (THR-132). It is
/// kept in the memory of the runner only: it is not saved, logged or sent to the hub (THR-051).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AccountRow {
    pub program: Program,
    #[serde(flatten)]
    pub facts: AccountFacts,
    /// The time of the newest report.
    pub at: chrono::DateTime<chrono::Utc>,
}

/// The models of one program, as its newest thread named them (TH104). In the memory of the
/// runner only: a program names its models when one of its threads starts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRow {
    pub program: Program,
    pub models: Vec<ModelChoice>,
    pub at: chrono::DateTime<chrono::Utc>,
}

/// Where the list of models of one program stands (TH117), so that a picker can say why a
/// program has no list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramState {
    pub program: Program,
    pub state: ListState,
    /// When the running ask started, or when the last ask failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListState {
    /// The program gave its list.
    Listed,
    /// An ask of the program runs.
    Asking,
    /// The last ask failed; the runner asks again a minute later.
    Failed,
    /// The program is not found on this computer.
    Missing,
    /// No ask ran yet: only a call of the person starts a program to ask it.
    NotAsked,
}
