//! Managed Code delivery shared by the hub, node broker and local execution owner.
//! Identities name authorization subjects; they are never credentials by themselves.
use crate::{
    control::{Action, Outcome},
    drive::Program,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub mod preview;
pub mod provider_login;
pub mod servers;
pub mod workspace;

pub const MAX_COMMAND_BYTES: usize = 48 * 1024;
pub const MAX_POLL_BYTES: usize = 1_100_000;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Command {
    pub request: Uuid,
    pub workspace: Uuid,
    pub generation: u64,
    pub principal: Uuid,
    pub action: Action,
}

impl Command {
    #[must_use]
    pub fn fits(&self) -> bool {
        if self.request.is_nil()
            || self.workspace.is_nil()
            || self.principal.is_nil()
            || !(1..=9_007_199_254_740_991).contains(&self.generation)
            || !serde_json::to_vec(self).is_ok_and(|bytes| bytes.len() <= MAX_COMMAND_BYTES)
        {
            return false;
        }
        match &self.action {
            Action::Start {
                program,
                folder,
                provider_address,
                program_path,
                draft,
                attachments,
                worktree,
                base,
                ..
            } => {
                matches!(program, Program::Codex | Program::ClaudeCode)
                    && folder == std::path::Path::new("/home/agent/workspace")
                    && provider_address.is_none()
                    && program_path.is_none()
                    && draft.is_none()
                    && attachments.is_empty()
                    && !worktree
                    && base.is_none()
            }
            Action::Send {
                thread,
                attachments,
                skills,
                ..
            } => !thread.is_nil() && attachments.is_empty() && skills.is_empty(),
            Action::Answer {
                thread, request, ..
            } => !thread.is_nil() && !request.is_nil(),
            Action::Interrupt { thread } | Action::Stop { thread } => !thread.is_nil(),
            _ => false,
        }
    }
}

impl Command {
    /// Results are restricted to the mutation's own response, never a transcript or account dump.
    #[must_use]
    pub fn accepts(&self, outcome: &Outcome) -> bool {
        match (outcome, &self.action) {
            (Outcome::Started { thread, folder }, Action::Start { .. }) => {
                *thread == self.request && folder == std::path::Path::new("/home/agent/workspace")
            }
            (Outcome::Refused { .. }, _)
            | (Outcome::Stopped { .. }, Action::Stop { .. })
            | (
                Outcome::Ok,
                Action::Send { .. }
                | Action::Answer { .. }
                | Action::Interrupt { .. }
                | Action::Stop { .. },
            ) => true,
            _ => false,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Offer {
    pub workspace: Uuid,
    pub generation: u64,
    pub ready: bool,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    pub request: Uuid,
    pub workspace: Uuid,
    pub generation: u64,
    pub principal: Uuid,
    pub result: Option<Outcome>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Poll {
    pub device: Uuid,
    /// Complete bounded inventory; missing workspaces cease being ready.
    pub workspaces: Vec<Offer>,
    pub reports: Vec<Report>,
}

impl Poll {
    #[must_use]
    pub fn fits(&self) -> bool {
        !self.device.is_nil()
            && self.workspaces.len() <= 128
            && self.reports.len() <= 1
            && self.workspaces.iter().all(|offer| {
                !offer.workspace.is_nil() && (1..=9_007_199_254_740_991).contains(&offer.generation)
            })
            && self
                .workspaces
                .iter()
                .map(|offer| offer.workspace)
                .collect::<std::collections::HashSet<_>>()
                .len()
                == self.workspaces.len()
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Delivery {
    pub command: Option<Command>,
    pub valid_until: Option<chrono::DateTime<chrono::Utc>>,
    pub acknowledged: Vec<Uuid>,
}

/// Fixed local broker; only the enrolled node daemon may connect.
pub const BROKER_SOCKET: &str = "/run/anima-execution/daemon.sock";
#[derive(Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum LocalRequest {
    Offers,
    OffersWithServers,
    ClosePreview {
        endpoint: Uuid,
        revision: u64,
        principal: Uuid,
    },
    Snapshot {
        workspace: Uuid,
        generation: u64,
        thread: Uuid,
        after: u64,
    },
    Deliver {
        command: Box<Command>,
        valid_until: chrono::DateTime<chrono::Utc>,
    },
    Status {
        workspace: Uuid,
        request: Uuid,
        principal: Uuid,
    },
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub row: ThreadSnapshot,
    pub updates: Vec<TranscriptUpdate>,
    pub current: Vec<serde_json::Value>,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ThreadSnapshot {
    pub id: Uuid,
    pub program: Program,
    pub state: crate::control::RunState,
    pub title: String,
    pub started: chrono::DateTime<chrono::Utc>,
    pub open: Vec<Uuid>,
}
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct TranscriptUpdate {
    pub seq: u64,
    pub content: Option<serde_json::Value>,
}
