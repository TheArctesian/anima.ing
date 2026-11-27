//! Boot recovery never starts a workspace or retries an uncertain build.
use super::{Error, Result, builds, workspace};
use anima_os_maintenance::{
    Admission, recovery,
    reservations::{self, RuntimeOwner},
};
use std::{collections::BTreeSet, fs, os::unix::fs::OpenOptionsExt, path::Path};
use uuid::Uuid;

pub async fn run() -> Result<()> {
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open("/run/anima-os/recovery.lock")?;
    lock.try_lock()
        .map_err(|_| Error::Refused("Node recovery is already running."))?;
    if recovery::ready()? {
        return Ok(());
    }
    super::source_jobs::recover().await?;
    let running = reconcile_workspaces().await?;
    for id in identities(builds::store::ROOT, false)? {
        builds::reconcile(id).await?;
    }
    let admission = Admission::cleanup()?;
    for reservation in reservations::list(&admission)? {
        if reservation.runtime == RuntimeOwner::Docker && running.contains(&reservation.run) {
            continue;
        }
        match reservation.runtime {
            RuntimeOwner::Docker => {
                reservations::release_stopped(&admission, reservation.run, reservation.generation)
                    .await?;
            }
            RuntimeOwner::SystemdBuild => {
                reservations::release_build(&admission, reservation.run, reservation.generation)
                    .await?;
            }
        }
    }
    recovery::complete(&admission)?;
    Ok(())
}

async fn reconcile_workspaces() -> Result<BTreeSet<Uuid>> {
    let mut running = BTreeSet::new();
    for id in identities("/persist/anima-workspaces", true)? {
        let state = workspace::record::required(id)?;
        if matches!(
            state.phase,
            workspace::record::Phase::Preparing | workspace::record::Phase::Prepared
        ) {
            continue;
        }
        if matches!(
            state.phase,
            workspace::record::Phase::Deleting | workspace::record::Phase::Deleted
        ) {
            recover_workspace_removal(state).await?;
            continue;
        }
        // A persisted stop intent must finish rather than turning back into running.
        let result = if state.phase == workspace::record::Phase::Stopping {
            workspace::stop(id, state.generation).await?
        } else {
            workspace::reconcile(id, state.generation).await?
        };
        if result["workspace"]["phase"] == "running" {
            running.insert(
                state
                    .run
                    .ok_or(Error::Refused("Recovered workspace has no run identity."))?,
            );
        }
    }
    Ok(running)
}

async fn recover_workspace_removal(state: workspace::record::Record) -> Result<()> {
    if let Some(binding) = state.handoff {
        if state.phase == workspace::record::Phase::Deleting {
            workspace::cleanup::retire(agent_threads::managed::workspace::handoff::Source {
                workspace: state.workspace,
                generation: state.generation,
                principal: state
                    .principal
                    .ok_or(Error::Refused("The retained source has no owner."))?,
                binding,
            })
            .await?;
        }
    } else {
        workspace::release(state.workspace, state.generation, true).await?;
    }
    Ok(())
}

fn identities(root: &str, files: bool) -> Result<Vec<Uuid>> {
    let mut ids = Vec::new();
    if !Path::new(root).try_exists()? {
        return Ok(ids);
    }
    for (count, entry) in fs::read_dir(root)?.enumerate() {
        if count >= 16384 {
            return Err(Error::Refused("Recovery inventory exceeded its bound."));
        }
        let path = entry?.path();
        if files && path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        if !files && path.file_name().is_some_and(|name| name == "locks") {
            continue;
        }
        let name = if files {
            path.file_stem()
        } else {
            path.file_name()
        };
        let id = name
            .and_then(|name| name.to_str())
            .and_then(|name| Uuid::parse_str(name).ok())
            .ok_or(Error::Refused(
                "Recovery inventory contains an invalid identity.",
            ))?;
        ids.push(id);
    }
    ids.sort();
    Ok(ids)
}
