//! Recent changes made through the authenticated local management session.
mod history;

use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::SystemTime,
};

use nix::unistd::{Uid, User};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use self::history::History;

const ROOT: &str = "/persist/anima-console-activity";

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Action {
    Connect,
    WifiOn,
    RemoteAccess,
    DisconnectRemote,
    HoldWork,
    ResumeWork,
    ReconcileMaintenance,
    Restart,
    Shutdown,
    DownloadUpdate,
    ScheduleUpdate,
    CancelScheduledUpdate,
    ActivateUpdate,
    RollbackUpdate,
    Enroll,
    KeepOrientation,
}

impl Action {
    pub(super) fn for_operation(operation: &str) -> Option<Self> {
        Some(match operation {
            "connect" => Self::Connect,
            "radio-on" => Self::WifiOn,
            "remote-access-set" | "remote-access-retry-defaults" => Self::RemoteAccess,
            "remote-disconnect" => Self::DisconnectRemote,
            "maintenance-drain-begin" => Self::HoldWork,
            "maintenance-drain-cancel" => Self::ResumeWork,
            "maintenance-release" => Self::ReconcileMaintenance,
            "restart" => Self::Restart,
            "shutdown" => Self::Shutdown,
            "update-download" => Self::DownloadUpdate,
            "update-schedule-activation" => Self::ScheduleUpdate,
            "update-cancel-activation" => Self::CancelScheduledUpdate,
            "update-activate" => Self::ActivateUpdate,
            "update-rollback" => Self::RollbackUpdate,
            "enroll" | "enroll-selfhost" => Self::Enroll,
            "keep-rotation" => Self::KeepOrientation,
            _ => return None,
        })
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Connect => "Connect to Wi-Fi",
            Self::WifiOn => "Turn Wi-Fi on",
            Self::RemoteAccess => "Change remote access",
            Self::DisconnectRemote => "Disconnect a remote session",
            Self::HoldWork => "Hold new work",
            Self::ResumeWork => "Resume work",
            Self::ReconcileMaintenance => "Reconcile maintenance",
            Self::Restart => "Restart",
            Self::Shutdown => "Shut down",
            Self::DownloadUpdate => "Download OS update",
            Self::ScheduleUpdate => "Schedule OS update",
            Self::CancelScheduledUpdate => "Cancel scheduled OS update",
            Self::ActivateUpdate => "Activate OS update",
            Self::RollbackUpdate => "Roll back OS update",
            Self::Enroll => "Enroll this computer",
            Self::KeepOrientation => "Save display orientation",
        }
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Outcome {
    Requested,
    Accepted,
    Failed,
}

impl Outcome {
    const fn label(self) -> &'static str {
        match self {
            Self::Requested => "Outcome unavailable",
            Self::Accepted => "Accepted",
            Self::Failed => "Reported failure",
        }
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Actor {
    uid: Option<u32>,
    account: Option<String>,
}

impl Actor {
    fn local() -> Self {
        // The installed wrapper is started by sudo. Resolve the sudo-assigned UID locally.
        let uid = match std::env::var("SUDO_UID") {
            Ok(value) => value.parse().ok(),
            Err(std::env::VarError::NotPresent) => Some(Uid::effective().as_raw()),
            Err(std::env::VarError::NotUnicode(_)) => None,
        };
        let account = uid
            .and_then(|uid| User::from_uid(Uid::from_raw(uid)).ok().flatten())
            .map(|user| {
                user.name
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(64)
                    .collect()
            });
        Self { uid, account }
    }

    fn label(&self) -> String {
        self.account
            .as_ref()
            .filter(|account| !account.is_empty())
            .cloned()
            .unwrap_or_else(|| {
                self.uid.map_or_else(
                    || "Unknown local account".to_owned(),
                    |uid| format!("UID {uid}"),
                )
            })
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    id: Uuid,
    actor: Actor,
    action: Action,
    at_ms: u64,
    outcome: Outcome,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    account: String,
    action: &'static str,
    at_ms: u64,
    outcome: &'static str,
}

#[derive(Serialize)]
pub(super) struct Snapshot {
    entries: Vec<Row>,
    complete: bool,
}

pub(super) struct OperatorActivity {
    actor: Actor,
    incomplete: AtomicBool,
}

impl OperatorActivity {
    pub(super) fn new() -> Self {
        Self {
            actor: Actor::local(),
            incomplete: AtomicBool::new(false),
        }
    }

    pub(super) async fn begin(&self, action: Action) -> Option<Uuid> {
        let id = Uuid::now_v7();
        let at_ms = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .and_then(|duration| u64::try_from(duration.as_millis()).ok())
            .unwrap_or_default();
        let entry = Entry {
            id,
            actor: self.actor.clone(),
            action,
            at_ms,
            outcome: Outcome::Requested,
        };
        let written = tokio::task::spawn_blocking(move || current().append(entry)).await;
        self.recorded(written.is_ok_and(|result| result.is_ok()))
            .then_some(id)
    }

    pub(super) async fn finish(&self, id: Uuid, accepted: bool) {
        let outcome = if accepted {
            Outcome::Accepted
        } else {
            Outcome::Failed
        };
        let written = tokio::task::spawn_blocking(move || current().finish(id, outcome)).await;
        self.recorded(written.is_ok_and(|result| result.is_ok()));
    }

    pub(super) async fn snapshot(&self) -> Option<Snapshot> {
        let entries = tokio::task::spawn_blocking(|| current().read())
            .await
            .ok()?
            .ok()?;
        Some(Snapshot {
            entries: entries
                .into_iter()
                .map(|entry| Row {
                    account: entry.actor.label(),
                    action: entry.action.label(),
                    at_ms: entry.at_ms,
                    outcome: entry.outcome.label(),
                })
                .collect(),
            complete: !self.incomplete.load(Ordering::Relaxed),
        })
    }

    fn recorded(&self, success: bool) -> bool {
        if !success && !self.incomplete.swap(true, Ordering::Relaxed) {
            eprintln!("anima-os-session: action history could not be saved");
        }
        success
    }
}

fn current() -> History<'static> {
    History::new(Path::new(ROOT), Uid::effective().as_raw())
}
