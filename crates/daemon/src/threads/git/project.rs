//! The Git folder of the repository of the person, read and written with no `git` (THR-050).
//!
//! An agent with a shell can write the configuration, the hooks and the `info` folder of this
//! Git folder, and each worktree shares them. A `git` that runs with this folder as its own
//! reads all of them, and a setting that is written while it runs is live at once. So the
//! runner never runs `git` with this folder as its Git folder for a command that could start
//! a program. Its own `git` uses a Git folder of the runner ([`super::place::own`]); what must
//! be read from or written to the folder of the person goes through this module, as files:
//! which branch a worktree is on, where a branch points, the move of a branch of the thread,
//! and the entry of a linked worktree.
//!
//! The one `git` that reads the configuration of the person's folder is [`settings`]: it
//! prints the settings and starts no program, whatever they say.
//!
//! I/O: reads and writes files below the Git folder. The readers of text are pure.

pub mod linked;
pub mod refs;

use std::io::Read as _;
use std::path::{Path, PathBuf};

use super::place::walk::{Dir, Make};
use super::run::settings::{self, Setting};
use super::run::{Git, GitError, Run};

/// The most bytes of a small file of a Git folder (`HEAD`, a loose ref, `commondir`).
const SMALL: u64 = 4096;

/// Why the Git folder of the person could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("the folder is not the top of a repository")]
    NotARepository,
    #[error("the repository keeps its refs in a form that Anima cannot write yet (reftable)")]
    Reftable,
    #[error(
        "the repository is a partial clone: it gets files from its remote only when they are \
         needed, so Anima cannot copy its history. Use a full clone"
    )]
    Partial,
    #[error("the path of the repository has a character that a Git file cannot hold")]
    OddPath,
    #[error("a file of the Git folder is not as git writes it: {0}")]
    Odd(&'static str),
    #[error("git or a different program holds a lock in the Git folder: {0}")]
    Locked(String),
    #[error("the branch moved while the action ran; nothing was written")]
    Moved,
    #[error("a path below the Git folder leaves it")]
    Escapes,
    #[error("the Git folder could not be read or written: {0}")]
    Io(#[from] std::io::Error),
}

impl From<ProjectError> for GitError {
    fn from(error: ProjectError) -> Self {
        Self::Failed {
            code: None,
            said: error.to_string(),
        }
    }
}

/// The two Git folders of one checkout: its own (`HEAD`, the index) and the shared one (the
/// refs, the objects). They are the same folder for the first checkout of a repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub git_dir: PathBuf,
    pub common_dir: PathBuf,
}

impl Layout {
    /// The Git folder of this checkout, held open. The entry of a linked worktree is found
    /// from the shared Git folder, one part at a time: a link in the place of the folder of
    /// the entries is not followed.
    ///
    /// # Errors
    /// The error of the system; `NotFound` when a part is no folder.
    pub fn entry(&self) -> std::io::Result<Dir> {
        match self.git_dir.strip_prefix(&self.common_dir) {
            Ok(inside) if inside.file_name().is_some() => Dir::open(&self.common_dir)?
                .below(inside, Make::No)?
                .ok_or_else(|| std::io::ErrorKind::NotFound.into()),
            _ => Dir::open(&self.git_dir),
        }
    }
}

/// The Git folders of the checkout whose top folder is `top`.
///
/// # Errors
/// [`ProjectError::NotARepository`] when `top` has no `.git` of a checkout;
/// [`ProjectError::Reftable`] and [`ProjectError::OddPath`] for a repository that the runner
/// cannot write to as files.
pub fn layout(top: &Path) -> Result<Layout, ProjectError> {
    let none = |_| ProjectError::NotARepository;
    let dot_git = top.join(".git");
    let about = std::fs::symlink_metadata(&dot_git).map_err(none)?;
    let git_dir = if about.is_dir() {
        dot_git
    } else {
        let pointer = small(&dot_git).map_err(none)?;
        let pointer = pointer.trim_end().strip_prefix("gitdir: ");
        top.join(pointer.ok_or(ProjectError::NotARepository)?)
    };
    let git_dir = std::fs::canonicalize(git_dir).map_err(none)?;
    let common_dir = match small(&git_dir.join("commondir")) {
        Ok(shared) => std::fs::canonicalize(git_dir.join(shared.trim_end())).map_err(none)?,
        Err(_) => git_dir.clone(),
    };
    if !common_dir.join("objects").is_dir() || !git_dir.join("HEAD").is_file() {
        return Err(ProjectError::NotARepository);
    }
    if common_dir.join("reftable").exists() {
        return Err(ProjectError::Reftable);
    }
    let promised = |file: std::fs::DirEntry| {
        Path::new(&file.file_name())
            .extension()
            .is_some_and(|kind| kind == "promisor")
    };
    let packs = std::fs::read_dir(common_dir.join("objects/pack"));
    if packs.is_ok_and(|packs| packs.flatten().any(promised)) {
        return Err(ProjectError::Partial);
    }
    // The path goes into a file of one line (`gitdir`) and into a list of object stores.
    let plain = |path: &Path| {
        path.to_str().is_some_and(|text| {
            // No `:` either: a list of object stores is split at it.
            !text.chars().any(|c| c.is_control() || c == ':') && !text.starts_with(['#', '"'])
        })
    };
    if !plain(&common_dir) || !plain(&git_dir) {
        return Err(ProjectError::OddPath);
    }
    Ok(Layout {
        git_dir,
        common_dir,
    })
}

/// Reads a file of at most `most` bytes. A link, a pipe or a folder at the path is not read:
/// an agent can put one there.
pub(super) fn limited(path: &Path, most: u64) -> std::io::Result<Vec<u8>> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        let flags = rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK;
        options.custom_flags(i32::try_from(flags.bits()).unwrap_or_default());
    }
    let file = options.open(path)?;
    let about = file.metadata()?;
    if !about.is_file() || about.len() > most {
        return Err(std::io::Error::other("not a file of the expected size"));
    }
    let mut bytes = Vec::new();
    file.take(most).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Reads a small file of text.
fn small(path: &Path) -> std::io::Result<String> {
    String::from_utf8(limited(path, SMALL)?).map_err(std::io::Error::other)
}

/// The settings that the `git` of the person sees in the worktree of `layout`, as text to
/// read: who the person is for a commit, and where the remotes are. `git config --list` only
/// prints; it starts no hook, no filter and no helper, and nothing that it prints is run.
///
/// # Errors
/// A [`GitError`] when the configuration cannot be listed or is not text.
pub async fn settings(git: &Git, layout: &Layout, folder: &Path) -> Result<Vec<Setting>, GitError> {
    let list = Run::new(folder, settings::LIST)
        .with_env("GIT_DIR", &layout.git_dir)
        .with_env("GIT_COMMON_DIR", &layout.common_dir);
    let listed = String::from_utf8(git.output(list).await?).map_err(|_| GitError::Failed {
        code: None,
        said: "the configuration of the repository has a name or a value that is not text"
            .to_owned(),
    })?;
    Ok(settings::read(&listed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_with_no_git_folder_is_no_repository() {
        let folder = tempfile::tempdir().expect("a folder");
        assert!(matches!(
            layout(folder.path()),
            Err(ProjectError::NotARepository)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_place_of_a_file_is_not_read() {
        let folder = tempfile::tempdir().expect("a folder");
        let real = folder.path().join("real");
        std::fs::write(&real, "ref: refs/heads/main\n").expect("a file");
        let link = folder.path().join("HEAD");
        std::os::unix::fs::symlink(&real, &link).expect("a link");
        assert!(small(&real).is_ok());
        assert!(small(&link).is_err());
    }
}
