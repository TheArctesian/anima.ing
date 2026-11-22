//! The queue of a thread (TH109): the prompts that wait for the turn to end, in the memory of the
//! runner only, and how the last turn ended.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The most prompts that wait in the queue of one thread (TH109).
pub const MOST_QUEUED: usize = 20;

/// The most bytes of one queued prompt (TH109): each update of the row carries the queue, so a
/// bound keeps the kept updates of the thread. A longer prompt can still be sent.
pub const MOST_QUEUED_BYTES: usize = 64 * 1024;

/// A prompt that waits in the queue of a thread (TH109).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Queued {
    pub id: Uuid,
    pub text: String,
}

/// How a turn ended, for the person (TH109).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LastTurn {
    Done,
    Failed,
    Interrupted,
}
