//! Local enrolled-daemon requests to the workspace owner.
use super::{Binding, Import, Receipt, Source};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PATH: &str = "/run/anima-workspace/handoff.sock";
pub const EXPORT: &str = "workspace-handoff-project-export/v1";
pub const RESTORE: &str = "workspace-handoff-project-restore/v1";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub valid_until: DateTime<Utc>,
    pub command: Command,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "operation", deny_unknown_fields)]
pub enum Command {
    #[serde(rename = "workspace-handoff-fence")]
    Fence {
        workspace: Uuid,
        generation: u64,
        principal: Uuid,
        binding: Binding,
    },
    #[serde(rename = "workspace-handoff-project-describe")]
    Describe { source: Source },
    #[serde(rename = "workspace-handoff-project-export")]
    Export { receipt: Receipt },
    #[serde(rename = "workspace-handoff-project-restore")]
    Restore { import: Import },
    #[serde(rename = "workspace-handoff-discard")]
    Discard { import: Import },
    #[serde(rename = "workspace-handoff-withdraw")]
    Withdraw { source: Source },
    #[serde(rename = "workspace-handoff-retire")]
    Retire { source: Source },
    #[serde(rename = "workspace-handoff-project-activate")]
    Activate { import: Import },
    #[serde(rename = "workspace-handoff-project-restore-status")]
    RestoreStatus { import: Import },
}
