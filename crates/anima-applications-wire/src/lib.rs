//! Local protocol between the enrolled daemon and the applications owner.
//!
//! Each frame is a big-endian 32-bit length followed by JSON. An item uses one
//! connection for inspection, the start decision, and the final result.
//! After `NeedsFiles`, an apply sends one length-prefixed Compose file and,
//! if the item names it, one length-prefixed environment file. Those two
//! frames contain the original bytes, including an optional empty file.

use serde::{Deserialize, Serialize};

pub const SOCKET: &str = "/run/anima-applications/control.sock";
pub const MAX_ITEM_BYTES: usize = 16 * 1024;
pub const MAX_REQUEST_BYTES: usize = 6 * MAX_ITEM_BYTES + 1024;
pub const MAX_COMPOSE_BYTES: usize = 256 * 1024;
pub const MAX_ENVIRONMENT_BYTES: usize = 64 * 1024;
pub const MAX_REPLY_BYTES: usize = 512;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    pub origin: String,
    pub device: String,
    pub command: Command,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Probe,
    ProbeApply,
    Item { json: String },
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Decision {
    Start,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reply {
    Paired,
    ApplyAvailable,
    NeedsFiles,
    Ready,
    Succeeded,
    CheckingHealth,
    Applied { health: Health },
    Refused { cause: String },
    Failed { cause: String },
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Health {
    Healthy,
    Unhealthy,
    NoCheck,
    NotWaited,
}
