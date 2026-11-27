//! Fetch a fixed commit inside a dedicated workspace.
mod credential;

use super::{Error, Result, command, record, runtime};
use crate::linux::work_policy;
pub(in crate::linux) use credential::{Credential, ManagedCredential};
use daemon_core::{
    wire::WorkRepository,
    work_repositories::{is_name, is_owner},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    repository: WorkRepository,
    commit: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    repository_id: Option<Uuid>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    source: Source,
    phase: Phase,
}

#[derive(PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Pending,
    Preparing,
    Ready,
    Interrupted,
}

impl Receipt {
    pub(super) fn captured_source(&self) -> Result<Source> {
        if !self.ready() {
            return Err(Error::Refused(
                "Finish source checkout before capturing the workspace.",
            ));
        }
        Ok(self.source.clone())
    }

    pub(super) fn pending(source: Source) -> Self {
        Self {
            source,
            phase: Phase::Pending,
        }
    }

    pub(super) fn restored(source: Source) -> Self {
        Self {
            source,
            phase: Phase::Ready,
        }
    }

    pub(super) const fn ready(&self) -> bool {
        matches!(self.phase, Phase::Ready)
    }

    pub(super) const fn source(&self) -> &Source {
        &self.source
    }

    pub(in crate::linux) fn view(&self) -> agent_threads::managed::workspace::Checkout {
        agent_threads::managed::workspace::Checkout {
            source: agent_threads::managed::workspace::Source {
                owner: self.source.repository.owner.clone(),
                repository: self.source.repository.name.clone(),
                commit: self.source.commit.clone(),
                repository_id: self.source.repository_id,
            },
            ready: self.ready(),
        }
    }
}

impl From<agent_threads::managed::workspace::Source> for Source {
    fn from(source: agent_threads::managed::workspace::Source) -> Self {
        Self {
            repository: WorkRepository {
                forge: "github".to_owned(),
                owner: source.owner,
                name: source.repository,
            },
            commit: source.commit,
            repository_id: source.repository_id,
        }
    }
}

impl Source {
    pub(in crate::linux) fn authorize(&self) -> Result<()> {
        self.validate()?;
        work_policy::authorize(&self.repository)
    }

    fn validate(&self) -> Result<()> {
        if self.repository.forge != "github"
            || !is_owner(&self.repository.owner)
            || !is_name(&self.repository.name)
            || self.repository_id.is_some_and(|id| id.is_nil())
            || self.commit.len() != 40
            || !self.commit.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(Error::Refused(
                "Choose a GitHub repository and a full commit identity.",
            ));
        }
        Ok(())
    }
}

pub async fn run(id: Uuid, generation: u64, source: Source) -> Result<Value> {
    checkout(id, generation, source, None).await
}

pub(in crate::linux) async fn checkout(
    id: Uuid,
    generation: u64,
    mut source: Source,
    credential: Option<Credential>,
) -> Result<Value> {
    source.validate()?;
    if source.repository_id.is_some() {
        return Err(Error::Refused(
            "A managed private checkout requires its workspace start authorization.",
        ));
    }
    source.commit.make_ascii_lowercase();
    work_policy::authorize(&source.repository)?;
    let _lock = record::lock(id)?;
    let mut state = record::required(id)?;
    require_workspace(&state, generation)?;
    if let Some(receipt) = &state.checkout {
        if receipt.source != source || !receipt.ready() {
            return Err(Error::Refused(
                "This checkout requires cleanup; it cannot be overwritten.",
            ));
        }
        return Ok(serde_json::to_value(receipt)?);
    }
    if runtime::observe(&state).await?["runtime"]["running"] != true {
        return Err(Error::Refused(
            "Reconcile the workspace before source checkout.",
        ));
    }
    download(&mut state, source, credential).await?;
    Ok(serde_json::to_value(state.checkout)?)
}

pub(super) fn allow_launch(
    state: &record::Record,
    credential: Option<&ManagedCredential>,
) -> Result<()> {
    authorize_credential(state, credential)?;
    let Some(receipt) = &state.checkout else {
        return Ok(());
    };
    if state.principal.is_none() {
        return Ok(());
    }
    if receipt.ready() || (receipt.phase == Phase::Pending && state.generation == 0) {
        return receipt.source.authorize();
    }
    Err(Error::Refused(
        "This workspace has an interrupted checkout. Reconcile its runtime and create a new workspace.",
    ))
}

fn authorize_credential(
    state: &record::Record,
    credential: Option<&ManagedCredential>,
) -> Result<()> {
    let required = state
        .checkout
        .as_ref()
        .filter(|receipt| state.principal.is_some() && receipt.phase == Phase::Pending)
        .and_then(|receipt| receipt.source.repository_id);
    if credential.map(ManagedCredential::repository_id) != required {
        return Err(Error::Refused(
            "The workspace requires its exact private checkout authorization.",
        ));
    }
    if let Some(credential) = credential {
        credential.check()?;
    }
    Ok(())
}

pub(super) async fn finish_planned(
    state: &mut record::Record,
    credential: Option<ManagedCredential>,
) -> Result<()> {
    let Some(receipt) = state
        .checkout
        .as_ref()
        .filter(|receipt| receipt.phase == Phase::Pending)
    else {
        return Ok(());
    };
    let source = receipt.source.clone();
    source.authorize()?;
    download(state, source, credential.map(ManagedCredential::into_git)).await.map_err(|_| Error::Refused(
        "The commit could not be checked out. Reconcile this workspace before starting another.",
    ))
}

async fn download(
    state: &mut record::Record,
    source: Source,
    credential: Option<Credential>,
) -> Result<()> {
    let directory = record::home(state.workspace).join("workspace");
    if !fs::symlink_metadata(&directory)?.is_dir() || fs::read_dir(&directory)?.next().is_some() {
        return Err(Error::Refused(
            "Source checkout requires an empty workspace directory.",
        ));
    }
    state.checkout = Some(Receipt {
        source: source.clone(),
        phase: Phase::Preparing,
    });
    record::save(state)?;
    let outcome = fetch(&state.container(), &source, credential)
        .await
        .and_then(|()| work_policy::authorize(&source.repository));
    state.checkout = Some(Receipt {
        source,
        phase: if outcome.is_ok() {
            Phase::Ready
        } else {
            Phase::Interrupted
        },
    });
    record::save(state)?;
    outcome
}

fn require_workspace(state: &record::Record, generation: u64) -> Result<()> {
    if state.generation != generation
        || generation != 1
        || state.principal.is_some()
        || state.phase != record::Phase::Running
    {
        return Err(Error::Refused(
            "Source checkout requires a fresh unassigned running workspace.",
        ));
    }
    Ok(())
}

async fn fetch(container: &str, source: &Source, credential: Option<Credential>) -> Result<()> {
    let url = format!(
        "https://github.com/{}/{}.git",
        source.repository.owner, source.repository.name
    );
    let mut args = vec!["exec"];
    if credential.is_some() {
        args.push("--interactive");
    }
    let seconds = credential.as_ref().map_or(Ok(45), Credential::seconds)?;
    let timeout = format!("{seconds}s");
    args.extend([
        "--user=1000:1000",
        container,
        "/bin/timeout",
        "--signal=TERM",
        "--kill-after=2s",
        &timeout,
        "/bin/env",
        "-i",
        "PATH=/bin",
        "HOME=/tmp",
        "LANG=C.UTF-8",
        "SSL_CERT_FILE=/etc/ssl/certs/ca-bundle.crt",
        "GIT_TERMINAL_PROMPT=0",
        "GIT_CONFIG_NOSYSTEM=1",
        "GIT_CONFIG_GLOBAL=/dev/null",
        "/bin/bash",
        "--noprofile",
        "--norc",
        "-c",
        include_str!("checkout.sh"),
        "anima-source-checkout",
        &url,
        &source.commit,
        if credential.is_some() {
            "private"
        } else {
            "public"
        },
    ]);
    let output = if let Some(credential) = credential {
        let input = credential.git_input();
        command::docker_input_for(&args, &input, seconds + 10).await?
    } else {
        command::docker(&args).await?
    };
    if output.trim() != source.commit {
        return Err(Error::Refused(
            "Source checkout did not return the requested commit.",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
