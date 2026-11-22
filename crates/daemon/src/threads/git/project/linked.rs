//! The entry of a linked worktree in the Git folder of the person, made and removed as files.
//! `git worktree add` in that folder would run its hooks and obey its configuration; three
//! small files and the `.git` file of the new folder are all that it writes.
//!
//! I/O: writes below `<shared Git folder>/worktrees` and makes the folder of the worktree.

use std::path::{Path, PathBuf};

use super::{Layout, ProjectError};

/// Makes the entry `name` of a worktree at `folder` on `branch`, and `folder` itself with its
/// `.git` file. Nothing is checked out. Gives the Git folders of the new worktree.
///
/// # Errors
/// [`ProjectError::Locked`] when an entry of this name is there already; the error of the
/// file system when a file cannot be written.
pub fn add(common: &Path, name: &str, folder: &Path, branch: &str) -> Result<Layout, ProjectError> {
    let entries = common.join("worktrees");
    std::fs::create_dir_all(&entries)?;
    // The entries are a real folder of this Git folder, not a link to a different place.
    if std::fs::canonicalize(&entries)? != entries {
        return Err(ProjectError::Escapes);
    }
    let entry = entries.join(name);
    std::fs::create_dir(&entry).map_err(|error| match error.kind() {
        std::io::ErrorKind::AlreadyExists => ProjectError::Locked(entry.display().to_string()),
        _ => ProjectError::Io(error),
    })?;
    let made = (|| {
        std::fs::create_dir(folder)?;
        let dot_git = folder.join(".git");
        std::fs::write(entry.join("HEAD"), format!("ref: refs/heads/{branch}\n"))?;
        std::fs::write(entry.join("commondir"), "../..\n")?;
        std::fs::write(entry.join("gitdir"), format!("{}\n", dot_git.display()))?;
        std::fs::write(dot_git, format!("gitdir: {}\n", entry.display()))
    })();
    if let Err(failed) = made {
        // The entry was made a moment ago by this call and names no work.
        let _ = std::fs::remove_dir_all(&entry);
        return Err(failed.into());
    }
    Ok(Layout {
        git_dir: entry,
        common_dir: common.to_owned(),
    })
}

/// Removes the entry of a worktree: the folder `git_dir`, only when it is directly below the
/// `worktrees` folder of `common`.
///
/// # Errors
/// [`ProjectError::Escapes`] for a different folder; the error of the file system.
pub fn remove(layout: &Layout) -> Result<(), ProjectError> {
    let entries: PathBuf = layout.common_dir.join("worktrees");
    if layout.git_dir.parent() != Some(entries.as_path()) {
        return Err(ProjectError::Escapes);
    }
    match std::fs::remove_dir_all(&layout.git_dir) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}
