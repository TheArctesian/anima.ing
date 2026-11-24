use std::path::Path;

use agent_threads::control::Refusal;
use reqwest::Url;

use super::refused;
use crate::threads::git::run::{Git, GitError, Run};

mod trace;

const MAX_URL_BYTES: usize = 2048;
const GIT_PATH: &str = "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin";
// Git's pack protocol interprets 0x7fff_ffff as infinite depth. A depth request refuses
// dumb HTTP before object or alternate lookups. Empty clones also need the trace check.
const FULL_DEPTH: &str = "--depth=2147483647";

pub(super) struct Source(Url);

impl Source {
    pub(super) fn parse(text: &str) -> Result<Self, Refusal> {
        let invalid =
            || refused("Enter an HTTPS Git URL without credentials, a query or a fragment.");
        if text.len() > MAX_URL_BYTES
            || text.contains(['\\', '?', '#'])
            || text.chars().any(|ch| ch.is_whitespace() || ch.is_control())
        {
            return Err(invalid());
        }
        let authority = text
            .strip_prefix("https://")
            .and_then(|rest| rest.split('/').next())
            .ok_or_else(invalid)?;
        if authority.is_empty() || authority.contains('@') {
            return Err(invalid());
        }
        Url::parse(text).map(Self).map_err(|_| invalid())
    }

    pub(super) async fn checkout(&self, folder: &Path) -> Result<(), Refusal> {
        checkout(folder, self.0.as_str(), "protocol.https.allow").await
    }
}

async fn checkout(folder: &Path, source: &str, protocol: &str) -> Result<(), Refusal> {
    let private = folder.with_file_name("home");
    let git = Git::find([
        ("HOME".into(), private.as_os_str().into()),
        ("PATH".into(), GIT_PATH.into()),
    ])
    .map_err(|_| refused("Git is required to clone a project. No project folder was created."))?;
    let run = Run::new(
        folder,
        [
            "clone",
            "--quiet",
            "--no-local",
            "--no-recurse-submodules",
            "--reject-shallow",
            "--no-single-branch",
            FULL_DEPTH,
            "--template=",
            "--",
            source,
            ".",
        ],
    )
    // Clone must find neither the user's settings nor a repository above the staging folder.
    .with_env("GIT_CONFIG_NOSYSTEM", "1")
    .with_env("GIT_CONFIG_GLOBAL", "/dev/null")
    .with_env("GIT_DIR", folder.with_file_name("unused-git-directory"))
    .with_setting(protocol, "always")
    .with_setting("http.followRedirects", "false");
    let (pid, received) = git.trace(run).await.map_err(|error| match error {
        GitError::TimedOut(_) => refused("The clone timed out. No project folder was created."),
        _ => refused("Git could not clone the repository. Check the URL and anonymous HTTPS access. No project folder was created."),
    })?;
    let transport = if source.starts_with("https://") {
        "remote-https"
    } else {
        "remote-http"
    };
    if !trace::negotiated(&received, pid, transport) {
        return Err(refused(
            "Git did not confirm smart HTTP negotiation. Use a Git version and HTTPS server that support it. No project folder was created.",
        ));
    }
    complete_history(folder).await
}

async fn complete_history(folder: &Path) -> Result<(), Refusal> {
    match tokio::fs::symlink_metadata(folder.join(".git/shallow")).await {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(refused(
            "This repository has shallow history. Choose a source with full history. No project folder was created.",
        )),
        Err(_) => Err(refused(
            "The cloned history could not be checked. No project folder was created.",
        )),
    }
}

#[cfg(test)]
mod tests;
