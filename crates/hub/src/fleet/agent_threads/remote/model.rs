use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::fleet) struct Owner {
    pub account: Uuid,
    pub device: Uuid,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::fleet) enum Program {
    ClaudeCode,
    Codex,
}

impl Program {
    pub(super) const fn word(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude_code",
            Self::Codex => "codex",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::fleet) struct Pair {
    pub project: Uuid,
    pub folder: String,
    pub program: Program,
}

impl Pair {
    pub(super) fn valid(&self) -> bool {
        self.folder.starts_with('/')
            && self.folder.len() <= 4096
            && !self.folder.contains('\0')
            && self
                .folder
                .split('/')
                .skip(1)
                .all(|part| !part.is_empty() && part != "." && part != "..")
            && !self.project.is_nil()
    }
}

#[derive(Serialize)]
pub(in crate::fleet) struct Start {
    pub launch: Uuid,
    pub pair: Pair,
    pub access: String,
    pub prompt: String,
}

impl Start {
    pub(super) fn valid(&self) -> bool {
        !self.launch.is_nil()
            && self.pair.valid()
            && matches!(
                self.access.as_str(),
                "full_access" | "accept_edits" | "ask" | "plan"
            )
            && !(self.access == "plan" && self.pair.program == Program::Codex)
            && super::commands::valid_prompt(&self.prompt)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::fleet) struct Target {
    pub device: Uuid,
    #[serde(flatten)]
    pub pair: Pair,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::fleet) struct OriginalClaim {
    pub device: Uuid,
    pub launch: Uuid,
    pub work: Uuid,
    pub attempt: u32,
}

#[derive(Deserialize)]
pub(in crate::fleet) struct Dispatch {
    #[serde(flatten)]
    pub claim: OriginalClaim,
    pub runner: Uuid,
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::fleet) enum EvidenceOutcome {
    Created,
    NotCreated,
}

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::fleet) enum PublicationUpdate {
    Withdrawn,
}

#[derive(Deserialize)]
pub(in crate::fleet) struct Evidence {
    #[serde(flatten)]
    pub claim: OriginalClaim,
    pub outcome: EvidenceOutcome,
    pub thread: Option<Uuid>,
    pub cause: Option<String>,
    pub publication: Option<PublicationUpdate>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::fleet) struct LaunchReceipt {
    pub launch: Uuid,
    pub state: &'static str,
    pub cause: Option<&'static str>,
    pub thread: Option<Uuid>,
    pub binding: &'static str,
    pub first_command: Uuid,
}

#[derive(sqlx::FromRow)]
pub(super) struct Launch {
    #[sqlx(rename = "launch_id")]
    pub id: Uuid,
    pub work_id: Uuid,
    pub device_id: Uuid,
    pub first_command: Uuid,
    pub fingerprint: Vec<u8>,
    pub key_version: i16,
    pub project: String,
    pub folder: String,
    pub program: String,
    pub access: String,
    pub deadline: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub lease_end: Option<DateTime<Utc>>,
    pub attempt_phase: Option<String>,
    pub runner_id: Option<Uuid>,
    pub authorization: Option<Uuid>,
    pub clock_generation: Option<Uuid>,
    pub issued_tick: Option<i64>,
    pub issued_at: Option<DateTime<Utc>>,
    pub valid_until: Option<DateTime<Utc>>,
    pub denial: Option<String>,
    pub evidence: Option<String>,
    pub admission_cause: Option<String>,
    pub publication: Option<String>,
    pub withdrawn: bool,
    pub control_closed: bool,
    pub device_gone: bool,
    pub thread_id: Option<Uuid>,
    pub retired: bool,
}

impl Launch {
    pub(super) fn receipt(&self, now: DateTime<Utc>) -> LaunchReceipt {
        let evidence = match self.evidence.as_deref() {
            Some("created") => Some("started"),
            Some("refused") => Some("refused"),
            Some("failed") => Some("failed"),
            Some("stopped") => Some("stopped"),
            Some("expired") => Some("expired"),
            _ => None,
        };
        let state = if self.retired {
            "retired"
        } else if let Some(proven) = evidence {
            proven
        } else if self.claimed_at.is_none() {
            "queued"
        } else if self.lease_end.is_some_and(|end| end > now)
            && self
                .claimed_at
                .is_some_and(|claim| claim + super::attempt::MAX_ATTEMPT > now)
            && now < self.deadline
        {
            "claimed"
        } else {
            "reconciling"
        };
        let (binding, cause, thread) = if self.withdrawn {
            ("unavailable", Some("sharing_withdrawn"), None)
        } else if self.device_gone {
            ("unavailable", Some("device_unavailable"), None)
        } else if self.control_closed {
            ("unavailable", Some("thread_ended"), None)
        } else if self.publication.as_deref() == Some("timed_out") {
            ("unavailable", Some("publication_timeout"), None)
        } else if let Some(thread) = self.thread_id {
            ("bound", None, Some(thread))
        } else {
            match evidence {
                Some(code) if code != "started" => {
                    ("unavailable", Some(self.refusal_cause(code)), None)
                }
                _ => ("waiting", None, None),
            }
        };
        LaunchReceipt {
            launch: self.id,
            state,
            cause,
            thread,
            binding,
            first_command: self.first_command,
        }
    }
    pub(super) fn refusal_cause<'a>(&self, otherwise: &'a str) -> &'a str {
        match self.admission_cause.as_deref().or(self.denial.as_deref()) {
            Some("target_unavailable") => "target_unavailable",
            Some("WORK_OFF") => "WORK_OFF",
            Some("LAUNCH_CONFLICT") => "LAUNCH_CONFLICT",
            _ => otherwise,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::fleet) struct Grant {
    pub authorization: Uuid,
    pub valid_until: DateTime<Utc>,
    pub remaining_ms: i64,
}

pub(in crate::fleet) enum DispatchDecision {
    Granted(Grant),
    Reconcile(Option<Uuid>),
    Denied(&'static str),
}
