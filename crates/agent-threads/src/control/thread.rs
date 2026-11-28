//! The non-secret state clients can read for one runner thread.
use super::{LastTurn, Queued, RunState, Workspace};
use crate::drive::{Effort, Mode, Program, Usage};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

/// The provider route fixed at thread start (TH118). No address or credential is accepted.
/// Local-model variants belong to the compute integration; they are not implemented here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderAddress {
    /// Experimental pass-through only, never shortening (R-400).
    Relay,
}

/// One thread of the runner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThreadRow {
    /// The route, never its secret address; absent in records from before TH118.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_address: Option<ProviderAddress>,
    pub id: Uuid,
    pub program: Program,
    pub folder: PathBuf,
    pub mode: Mode,
    pub model: Option<String>,
    pub state: RunState,
    /// The session ID of Claude Code or the thread ID of Codex, when the program named it.
    pub session: Option<String>,
    pub title: String,
    pub started: chrono::DateTime<chrono::Utc>,
    /// Why the thread stopped, in words for the person.
    pub ended: Option<String>,
    /// The requests that wait for the person.
    pub open: Vec<Uuid>,
    /// The prompts that wait for the turn to end, oldest first (TH109). In the memory of the
    /// runner only: the record never holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued: Vec<Queued>,
    /// How the last turn ended, when one ended (TH109).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_turn: Option<LastTurn>,
    /// The effort of the model, when the person chose one (TH110).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<Effort>,
    /// The speed of the model, when the person chose one other than the standard (TH117).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<String>,
    /// Where the thread works, read at its start (TH112).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<Workspace>,
    /// The use of tokens that the program last said (TH112). It also comes live, as a
    /// [`DriveEvent::Usage`]; the row keeps it for a client that comes later.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// The folder of the threads of the daemon: `threads` below the folder of the daemon, which is
/// `ANIMA_DAEMON_HOME`, else on macOS `~/Library/Application Support/anima-daemon`, else
/// `$XDG_CONFIG_HOME/anima-daemon` or `~/.config/anima-daemon` (as `Home::locate` of the daemon).
#[must_use]
pub fn folder(env: impl Fn(&str) -> Option<String>, macos: bool) -> Option<PathBuf> {
    let daemon = if let Some(set) = env("ANIMA_DAEMON_HOME") {
        PathBuf::from(set)
    } else if macos {
        Path::new(&env("HOME")?).join("Library/Application Support/anima-daemon")
    } else if let Some(config) = env("XDG_CONFIG_HOME").filter(|dir| !dir.is_empty()) {
        Path::new(&config).join("anima-daemon")
    } else {
        Path::new(&env("HOME")?).join(".config/anima-daemon")
    };
    Some(daemon.join("threads"))
}
