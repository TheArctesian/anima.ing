//! Closed lifecycle API for managed workspaces; contains no host paths or credentials.
use daemon_core::work_repositories::{is_name, is_owner};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod handoff;

pub const SOCKET: &str = "/run/anima-workspace/control.sock";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub request: Uuid,
    pub workspace: Uuid,
    pub principal: Uuid,
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Action {
    Delete {
        generation: u64,
        confirmed: bool,
    },
    Prepare {
        storage_bytes: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        source: Option<Source>,
    },
    Start {
        generation: u64,
        memory_bytes: u64,
        cpu_millis: u64,
        tasks: u64,
    },
    Stop {
        generation: u64,
    },
    Reconcile {
        generation: u64,
    },
}

/// A GitHub repository at one exact commit. A connected repository ID requires private checkout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub owner: String,
    pub repository: String,
    pub commit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_id: Option<Uuid>,
}

impl Source {
    #[must_use]
    pub fn fits(&self) -> bool {
        is_owner(&self.owner)
            && is_name(&self.repository)
            && self.repository_id.is_none_or(|id| !id.is_nil())
            && self.commit.len() == 40
            && self
                .commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    }
}

impl Command {
    #[must_use]
    pub fn fits(&self) -> bool {
        if self.request.is_nil() || self.workspace.is_nil() || self.principal.is_nil() {
            return false;
        }
        let valid_generation = |generation: u64| (1..=9_007_199_254_740_991).contains(&generation);
        match &self.action {
            Action::Delete {
                generation,
                confirmed,
            } => *confirmed && *generation <= 9_007_199_254_740_991,
            Action::Prepare {
                storage_bytes,
                source,
            } => {
                (1 << 30..=64 << 30).contains(storage_bytes)
                    && source.as_ref().is_none_or(Source::fits)
            }
            Action::Start {
                generation,
                memory_bytes,
                cpu_millis,
                tasks,
            } => {
                valid_generation(*generation)
                    && (64 << 20..=64 << 30).contains(memory_bytes)
                    && (1..=64_000).contains(cpu_millis)
                    && (1..=512).contains(tasks)
            }
            Action::Stop { generation } | Action::Reconcile { generation } => {
                valid_generation(*generation)
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum LocalRequest {
    Ready,
    Deliver {
        command: Command,
        valid_until: chrono::DateTime<chrono::Utc>,
    },
    /// The checkout credential follows the JSON header as a bounded raw frame.
    DeliverPrivate {
        command: Command,
        valid_until: chrono::DateTime<chrono::Utc>,
        checkout: CheckoutGrant,
    },
    Status {
        workspace: Uuid,
        request: Uuid,
        principal: Uuid,
    },
    Inspect {
        workspace: Uuid,
        principal: Uuid,
    },
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckoutGrant {
    pub repository_id: Uuid,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct View {
    pub workspace: Uuid,
    pub generation: u64,
    pub phase: Phase,
    pub storage_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<Checkout>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Checkout {
    pub source: Source,
    pub ready: bool,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Preparing,
    Prepared,
    Starting,
    Running,
    Stopping,
    Stopped,
    Unknown,
    Deleting,
    Deleted,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub command: Command,
    /// None means the final outcome is unresolved; never replay automatically.
    pub result: Option<View>,
    #[serde(default)]
    pub failure: Option<String>,
}

pub const MAX_POLL_BYTES: usize = 8192;

impl Receipt {
    #[must_use]
    pub fn fits(&self) -> bool {
        self.command.fits()
            && self.failure.as_ref().is_none_or(|text| text.len() <= 512)
            && !(self.result.is_some() && self.failure.is_some())
            && self.result.as_ref().is_none_or(|view| {
                view.workspace == self.command.workspace
                    && view.generation <= 9_007_199_254_740_991
                    && ((matches!(self.command.action, Action::Delete { .. })
                        && matches!(view.phase, Phase::Deleted)
                        && view.storage_bytes == 0)
                        || (!matches!(view.phase, Phase::Deleted)
                            && (1 << 30..=64 << 30).contains(&view.storage_bytes)))
                    && view.checkout.as_ref().is_none_or(|checkout| {
                        checkout.source.fits()
                            && (!matches!(view.phase, Phase::Running) || checkout.ready)
                    })
                    && match &self.command.action {
                        Action::Delete { generation, .. } => {
                            view.generation == *generation && matches!(view.phase, Phase::Deleted)
                        }
                        Action::Prepare {
                            storage_bytes,
                            source,
                        } => {
                            view.storage_bytes == *storage_bytes
                                && view.checkout.as_ref().map(|checkout| &checkout.source)
                                    == source.as_ref()
                        }
                        Action::Start { generation, .. } | Action::Stop { generation } => {
                            view.generation == *generation
                        }
                        Action::Reconcile { generation } => {
                            view.generation == *generation
                                || (matches!(view.phase, Phase::Prepared | Phase::Stopped)
                                    && view.generation.checked_add(1) == Some(*generation))
                        }
                    }
            })
    }
}

/// Headroom observed by the node's admission owner. A poll does not reserve it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capacity {
    pub storage_bytes: u64,
    pub memory_bytes: u64,
    pub cpu_millis: u64,
    pub tasks: u64,
}

impl Capacity {
    pub const MAXIMUM: Self = Self {
        storage_bytes: 64 << 30,
        memory_bytes: 64 << 30,
        cpu_millis: 64_000,
        tasks: 512,
    };

    #[must_use]
    pub const fn fits(self) -> bool {
        self.storage_bytes <= Self::MAXIMUM.storage_bytes
            && self.memory_bytes <= Self::MAXIMUM.memory_bytes
            && self.cpu_millis <= Self::MAXIMUM.cpu_millis
            && self.tasks <= Self::MAXIMUM.tasks
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapacityPoll {
    pub device: Uuid,
    pub ready: bool,
    pub report: Option<Receipt>,
    pub capacity: Capacity,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Poll {
    pub device: Uuid,
    pub ready: bool,
    pub report: Option<Receipt>,
}
impl Poll {
    #[must_use]
    pub fn fits(&self) -> bool {
        !self.device.is_nil() && self.report.as_ref().is_none_or(Receipt::fits)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Delivery {
    pub command: Option<Command>,
    pub valid_until: Option<chrono::DateTime<chrono::Utc>>,
    pub acknowledged: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout_repository: Option<Uuid>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckoutRequest {
    pub request: Uuid,
    pub repository_id: Uuid,
}

#[cfg(test)]
mod tests;
