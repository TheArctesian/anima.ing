//! This module adds the caller check, the installed `anima` and the error codes (MCP-039).
//! It starts no `anima` program and reads no token.

use anima_cli::mcp::install::setup::{Action, Here, check_command};
use anima_cli::mcp::install::{Asked, Row, change, listing};
use serde::{Serialize, Serializer, ser::SerializeStruct};
use tauri::{Runtime, Webview};
use tauri_plugin_log::log;

use crate::command_line;

/// Carries no path and no file text (LOG-007).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AgentsError {
    #[error("only the main window may ask")]
    Caller,
    #[error("the command line tool is not installed")]
    NoCommandLine,
    #[error("the installed anima is not a fixed program")]
    Command,
    #[error("the home folder does not belong to this user")]
    User,
    #[error("the coding agents could not be read or changed")]
    Failed,
}

impl AgentsError {
    /// Stable code the client switches on.
    const fn code(self) -> &'static str {
        match self {
            Self::Caller => "caller",
            Self::NoCommandLine => "no_command_line",
            Self::Command => "command",
            Self::User => "user",
            Self::Failed => "failed",
        }
    }
}

/// Serialises as `{ "code": ..., "message": ... }` with the fixed `Display` text.
impl Serialize for AgentsError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut out = serializer.serialize_struct("AgentsError", 2)?;
        out.serialize_field("code", self.code())?;
        out.serialize_field("message", &self.to_string())?;
        out.end()
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wanted {
    uninstall: bool,
    #[serde(default)]
    agents: Vec<String>,
    #[serde(default)]
    force: bool,
}

static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

async fn with<R: Runtime>(
    webview: Webview<R>,
    job: impl FnOnce(&Here, &anima_cli::mcp::install::setup::Checked) -> Vec<Row> + Send + 'static,
) -> Result<Vec<Row>, AgentsError> {
    crate::web_view::caller(webview.label()).map_err(|_| AgentsError::Caller)?;
    let installed = command_line::installed()
        .await
        .ok_or(AgentsError::NoCommandLine)?;
    let done = tauri::async_runtime::spawn_blocking(move || {
        let here = Here::for_this_user(installed.home, installed.path).ok_or(AgentsError::User)?;
        let anima = check_command(&installed.anima, here.user).map_err(|_| AgentsError::Command)?;
        if let Some(runs) = &installed.runs {
            check_command(runs, here.user).map_err(|_| AgentsError::Command)?;
        }
        // One read or change at a time: two changes of one agent's own program could race.
        let _one = ONE_AT_A_TIME
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Ok(job(&here, &anima))
    })
    .await
    .unwrap_or(Err(AgentsError::Failed));
    if let Err(error) = &done {
        log::warn!(
            "the coding agents were not read or changed: {}",
            error.code()
        );
    }
    done
}

const fn action(uninstall: bool) -> Action {
    if uninstall {
        Action::Uninstall
    } else {
        Action::Install
    }
}

/// Changes nothing (MCP-027, MCP-029).
#[tauri::command]
pub async fn mcp_agents_read<R: Runtime>(
    webview: Webview<R>,
    uninstall: bool,
) -> Result<Vec<Row>, AgentsError> {
    with(webview, move |here, anima| {
        listing(here, anima, action(uninstall))
    })
    .await
}

/// One refused agent does not stop the others; its row says why (MCP-028, MCP-036).
#[tauri::command]
pub async fn mcp_agents_change<R: Runtime>(
    webview: Webview<R>,
    wanted: Wanted,
) -> Result<Vec<Row>, AgentsError> {
    // The page names each listed agent; "all" and "force" are for the command line (MCP-029,
    // MCP-036).
    if wanted.agents.is_empty() || wanted.force {
        return Err(AgentsError::Failed);
    }
    let result = with(webview, move |here, anima| {
        change(
            here,
            anima,
            &Asked {
                action: action(wanted.uninstall),
                ids: &wanted.agents,
                force: wanted.force,
            },
        )
    })
    .await;
    if let Ok(rows) = &result {
        log::info!(
            "the coding agents were changed; {} refused",
            rows.iter().filter(|row| row.problem.is_some()).count()
        );
    }
    result
}
