use std::path::{Component, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use agent_threads::control::Refusal;

use super::refused;

pub(super) async fn project(folder: PathBuf, daemon: PathBuf) -> Result<PathBuf, Refusal> {
    let folder = tokio::task::spawn_blocking(move || destination(&folder, &daemon))
        .await
        .map_err(|_| refused("The project folder could not be checked."))??;
    tokio::fs::create_dir(&folder).await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::AlreadyExists {
            refused("That path already exists. Choose a new folder, or add the existing folder as a local project.")
        } else {
            refused("The new folder could not be created. Check the parent folder and its permissions.")
        }
    })?;
    initialize(&folder).await?;
    Ok(folder)
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
        .map_err(|_| refused("The folder was created, but Git could not start. Install Git, then initialize and add this folder."))?;
    match tokio::time::timeout(Duration::from_secs(30), child.wait()).await {
        Ok(Ok(status)) if status.success() => Ok(()),
        _ => {
            let _ = child.kill().await;
            Err(refused(
                "The folder was created, but Git initialization failed or timed out. The folder was kept; initialize and add it, or choose another new folder.",
            ))
        }
    }
}

#[cfg(test)]
mod tests;
