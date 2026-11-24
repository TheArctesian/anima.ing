use std::path::Path;

use agent_threads::control::Refusal;
use reqwest::Url;

use super::refused;
use crate::threads::git::run::{Git, GitError, Run};

const MAX_URL_BYTES: usize = 2048;
const GIT_PATH: &str = "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin";
// Git's pack protocol interprets 0x7fff_ffff as infinite depth. Requesting depth rules out
// dumb HTTP and its HTTP alternates list while keeping the complete history on smart HTTPS.
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
    git.output(run).await.map(drop).map_err(|error| match error {
        GitError::TimedOut(_) => refused("The clone timed out. No project folder was created."),
        _ => refused("Git could not clone the repository. Check the URL and anonymous HTTPS access. No project folder was created."),
    })
}

#[cfg(test)]
mod tests;
