//! The owner opts this computer into private SSH; this never runs as a hub or MCP request.

use crate::{failure::Failure, invocation::Invocation, output::Table};
use clap::Args;
use serde_json::json;
use std::{path::PathBuf, time::Duration};

#[derive(Debug, Args)]
pub struct EnableArgs {
    /// Advanced: the enrolled anima-link state folder.
    #[arg(long)]
    state: Option<PathBuf>,
    /// Advanced: the installed anima-link executable.
    #[arg(long)]
    link_program: Option<PathBuf>,
}

impl EnableArgs {
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        if !invocation.prompter.interactive() {
            return Err(Failure::Usage { message: "Run this command in a terminal so your operating system can ask for administrator approval.".into(), hint: None });
        }
        let application =
            application().ok_or_else(|| failed("Could not find the Anima application folder."))?;
        let state = self
            .state
            .unwrap_or_else(|| application.join("link/device"));
        let binary = if cfg!(windows) {
            "anima-link.exe"
        } else {
            "anima-link"
        };
        let program = self
            .link_program
            .unwrap_or_else(|| application.join("link/program").join(binary));
        if !state.is_dir() || !program.is_file() {
            return Err(failed(
                "Sign this computer into Anima first, or provide --state and --link-program for a standalone installation.",
            ));
        }
        invocation.output.note(
            "Setting up private SSH. Your operating system may ask for administrator approval.",
        );
        let user = remote_login::enable(remote_login::Prompt::Terminal)
            .await
            .map_err(|error| failed(&error.to_string()))?;
        let mut command = tokio::process::Command::new(program);
        command
            .arg("ports")
            .arg("-state")
            .arg(state)
            .args(["on", "22"])
            .kill_on_drop(true);
        let answer = tokio::time::timeout(Duration::from_secs(5), command.output())
            .await
            .map_err(|_| failed("SSH is ready, but enabling the private link port timed out."))?
            .map_err(|_| failed("SSH is ready, but the private link could not be started."))?;
        if !answer.status.success() {
            return Err(failed(
                "SSH is ready, but the private link refused the port. Check this computer's enrollment.",
            ));
        }
        invocation.output.data(
            &json!({"username": user, "port": 22, "ready": true}),
            |_| {
                let mut table = Table::record();
                table.pair("Private SSH", "Ready");
                table.pair("User name", user);
                table
            },
        )
    }
}

fn failed(message: &str) -> Failure {
    Failure::Ended {
        message: message.into(),
        status: 1,
    }
}

fn application() -> Option<PathBuf> {
    if cfg!(windows) {
        return std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .map(|p| p.join("Anima"));
    }
    let home = PathBuf::from(std::env::var_os("HOME")?);
    if !home.is_absolute() {
        return None;
    }
    if cfg!(target_os = "macos") {
        return Some(home.join("Library/Application Support/ing.anima.desktop"));
    }
    Some(
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"))
            .join("anima"),
    )
}
