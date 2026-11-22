use super::{probe::Diagnostics, status::Status};
use serde::{Deserialize, Serialize};

pub const VERSION: u8 = 1;
pub const REQUEST_LIMIT: usize = 1024;
pub const RESPONSE_LIMIT: usize = 16384;

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Status,
    Diagnostics,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u8,
    pub operation: Operation,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub version: u8,
    #[serde(flatten)]
    pub outcome: Outcome,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "result", content = "value", rename_all = "snake_case")]
pub enum Outcome {
    Status(Status),
    Diagnostics(Diagnostics),
    Error(Failure),
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    InvalidRequest,
    UnsupportedVersion,
    RequestTooLarge,
    TimedOut,
}

impl Reply {
    pub fn new(outcome: Outcome) -> Self {
        Self {
            version: VERSION,
            outcome,
        }
    }

    pub fn failure(failure: Failure) -> Self {
        Self::new(Outcome::Error(failure))
    }
}
