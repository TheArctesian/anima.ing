use std::path::{Component, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use agent_threads::control::Refusal;

use super::refused;

mod place;
pub(super) use place::Project;

pub(super) enum Preparation {
    Initialize,
    Clone(super::clone::Source),
}

pub(super) async fn project(
    folder: PathBuf,
    daemon: PathBuf,
    preparation: Preparation,
) -> Result<Project, Refusal> {
    let (destination, staging) = tokio::task::spawn_blocking(move || {
        let destination = place::Destination::open(destination(&folder, &daemon)?)?;
        // Git only sees a private staging directory inside the daemon boundary. The requested
        // pathname is never a subprocess cwd; publishing is an exclusive, handle-relative rename.
        let staging = tempfile::Builder::new()
            .prefix("project-")
            .tempdir_in(&daemon)
            .map_err(|_| {
                refused("The new project could not be prepared in Anima's private folder.")
            })?;
        Ok::<_, Refusal>((destination, staging))
    })
    .await
    .map_err(|_| refused("The project folder could not be checked."))??;
    let staged = staging.path().join("repository");
    tokio::fs::create_dir(&staged)
        .await
        .map_err(|_| refused("The new project could not be prepared."))?;
    match preparation {
        Preparation::Initialize => initialize(&staged).await?,
        Preparation::Clone(source) => source.checkout(&staged).await?,
    }
    destination.publish(&staged)
}

fn destination(folder: &std::path::Path, daemon: &std::path::Path) -> Result<PathBuf, Refusal> {
    if !folder.is_absolute()
        || folder
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(Refusal::FolderNotPermitted);
    }
    let name = folder.file_name().ok_or(Refusal::FolderNotPermitted)?;
    let parent = folder.parent().ok_or(Refusal::FolderNotPermitted)?;
    let real = crate::threads::notice::chosen_folder(parent, daemon)?;
    Ok(real.join(name))
}

async fn initialize(folder: &std::path::Path) -> Result<(), Refusal> {
    let mut child = tokio::process::Command::new("git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/local/bin:/opt/homebrew/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .arg("-c").arg("core.hooksPath=/dev/null")
        .args(["init", "--quiet", "--template=", "--initial-branch=main"])
        .current_dir(folder)
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| refused("Git could not start. Install Git, then create the project again. No project folder was created."))?;
    match tokio::time::timeout(Duration::from_secs(30), child.wait()).await {
        Ok(Ok(status)) if status.success() => Ok(()),
        _ => {
            let _ = child.kill().await;
            Err(refused(
                "Git initialization failed or timed out. No project folder was created; try creating it again.",
            ))
        }
    }
}

#[cfg(test)]
mod tests;
