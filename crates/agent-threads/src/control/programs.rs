//! What the programs of this computer last said of themselves: their accounts (THR-132) and their
//! models (TH104). In the memory of the runner only.

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
