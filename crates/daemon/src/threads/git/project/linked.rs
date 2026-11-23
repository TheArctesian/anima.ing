//! The entry of a linked worktree in the Git folder of the person, made and removed as files.
//! `git worktree add` in that folder would run its hooks and obey its configuration; three
//! small files and the `.git` file of the new folder are all that it writes.
//!
//! I/O: writes below `<shared Git folder>/worktrees`, found from the open Git folder with no
//! link followed ([`walk`]), and makes the folder of the worktree.

use std::io::Write as _;
use std::path::Path;

use super::super::place::walk::{self, Dir, Make};
use super::{Layout, ProjectError};

/// The folder of the entries of the worktrees in a shared Git folder.
const ENTRIES: &str = "worktrees";

/// Makes the entry `name` of a worktree at `folder` on `branch`, and `folder` itself with its
/// `.git` file. Nothing is checked out. Gives the Git folders of the new worktree.
///
/// # Errors
/// [`ProjectError::Locked`] when an entry of this name is there already; the error of the
/// file system when a file cannot be written.
pub fn add(common: &Path, name: &str, folder: &Path, branch: &str) -> Result<Layout, ProjectError> {
    let entries = Dir::open(common)?
        .below(Path::new(ENTRIES), Make::Missing)
        .map_err(|_| ProjectError::Escapes)?
        .ok_or(ProjectError::Escapes)?;
    let entry = common.join(ENTRIES).join(name);
    let named = walk::name(name);
    entries
        .new_folder(named)
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => ProjectError::Locked(entry.display().to_string()),
            _ => ProjectError::Io(error),
        })?;
    let made = (|| {
        let inside = entries
            .below(Path::new(name), Make::No)?
            .ok_or(std::io::ErrorKind::NotFound)?;
        // The folder of the worktree is made in its open folder, and its `.git` file in it.
        let (above, last) = (folder.parent(), folder.file_name());
        let (Some(above), Some(last)) = (above, last) else {
            return Err(std::io::ErrorKind::InvalidInput.into());
        };
        let above = Dir::open(above)?;
        above.new_folder(last)?;
        let made = above
            .below(Path::new(last), Make::No)?
            .ok_or(std::io::ErrorKind::NotFound)?;
        let dot_git = folder.join(".git");
        let texts = [
            ("HEAD", format!("ref: refs/heads/{branch}\n")),
            ("commondir", "../..\n".to_owned()),
            ("gitdir", format!("{}\n", dot_git.display())),
        ];
        for (file, text) in texts {
            inside
                .new_file(walk::name(file), false)?
                .write_all(text.as_bytes())?;
        }
        made.new_file(walk::name(".git"), false)?
            .write_all(format!("gitdir: {}\n", entry.display()).as_bytes())
    })();
    if let Err(failed) = made {
        // The entry was made a moment ago by this call and names no work.
        let _ = entries.remove_all(named);
        return Err(failed.into());
    }
    Ok(Layout {
        git_dir: entry,
        common_dir: common.to_owned(),
    })
}

/// Removes the entry of a worktree: the folder `git_dir`, only when it is directly below the
/// `worktrees` folder of `common`. A link in it is removed, never followed.
///
/// # Errors
/// [`ProjectError::Escapes`] for a different folder; the error of the file system.
pub fn remove(layout: &Layout) -> Result<(), ProjectError> {
    let entries = layout.common_dir.join(ENTRIES);
    let name = layout.git_dir.file_name();
    let (Some(name), true) = (name, layout.git_dir.parent() == Some(entries.as_path())) else {
        return Err(ProjectError::Escapes);
    };
    let Some(entries) = Dir::open(&layout.common_dir)?.below(Path::new(ENTRIES), Make::No)? else {
        return Ok(());
    };
    Ok(entries.remove_all(name)?)
}
