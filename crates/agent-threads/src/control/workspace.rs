//! Where a thread works, as the details card of the client shows it (TH112): a Git checkout, a
//! linked worktree, or a folder with no Git. The runner reads it once at the start, from the
//! folder of the thread upward, with no process of Git.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What holds the folder of a thread.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceKind {
    /// A repository whose `.git` is a folder.
    Checkout,
    /// A linked worktree: its `.git` is a file that names the folder of the repository.
    Worktree,
    /// No `.git` in the folder or above it.
    Folder,
}

/// The kind of the workspace, and its top: the folder that holds `.git`, or the folder of the
/// thread when there is no Git.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    pub kind: WorkspaceKind,
    pub top: PathBuf,
}

/// The most bytes read of a `.git` file: it holds one line, `gitdir: <path>`.
const MOST_GIT_FILE_BYTES: u64 = 4096;

impl Workspace {
    /// The workspace of `folder`. A `.git` folder is a checkout. A `.git` file is a linked
    /// worktree when it names a folder below `worktrees/` of a repository; else it is a
    /// submodule, which is a checkout too.
    #[must_use]
    pub fn of(folder: &Path) -> Self {
        for top in folder.ancestors() {
            let git = top.join(".git");
            let Ok(found) = std::fs::symlink_metadata(&git) else {
                continue;
            };
            let kind = if found.is_file() && is_worktree(&git) {
                WorkspaceKind::Worktree
            } else {
                WorkspaceKind::Checkout
            };
            return Self {
                kind,
                top: top.to_owned(),
            };
        }
        Self {
            kind: WorkspaceKind::Folder,
            top: folder.to_owned(),
        }
    }
}

fn is_worktree(git: &Path) -> bool {
    let mut text = String::new();
    let read = std::fs::File::open(git)
        .and_then(|file| file.take(MOST_GIT_FILE_BYTES).read_to_string(&mut text));
    if read.is_err() {
        return false;
    }
    text.lines()
        .find_map(|line| line.strip_prefix("gitdir:"))
        .map(|named| Path::new(named.trim()))
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .is_some_and(|name| name == "worktrees")
}

#[cfg(test)]
mod tests {
    use super::{Workspace, WorkspaceKind};

    #[test]
    fn th112_a_folder_is_a_checkout_a_worktree_or_no_git() {
        let root = tempfile::tempdir().expect("a folder");
        let repo = root.path().join("repo");
        std::fs::create_dir_all(repo.join(".git/worktrees/side")).expect("a repository");
        std::fs::create_dir_all(repo.join("src/deep")).expect("a folder");
        let found = Workspace::of(&repo.join("src/deep"));
        assert_eq!(
            (found.kind, found.top),
            (WorkspaceKind::Checkout, repo.clone())
        );

        let side = root.path().join("side");
        std::fs::create_dir_all(&side).expect("a folder");
        let gitdir = repo.join(".git/worktrees/side");
        std::fs::write(side.join(".git"), format!("gitdir: {}\n", gitdir.display()))
            .expect("a .git file");
        assert_eq!(Workspace::of(&side).kind, WorkspaceKind::Worktree);

        let module = root.path().join("module");
        std::fs::create_dir_all(&module).expect("a folder");
        std::fs::write(module.join(".git"), "gitdir: ../repo/.git/modules/module\n")
            .expect("a .git file");
        assert_eq!(
            Workspace::of(&module).kind,
            WorkspaceKind::Checkout,
            "a submodule"
        );

        let linked = root.path().join("linked");
        std::fs::create_dir_all(&linked).expect("a folder");
        std::os::unix::fs::symlink(side.join(".git"), linked.join(".git")).expect("a link");
        assert_eq!(
            Workspace::of(&linked).kind,
            WorkspaceKind::Checkout,
            "a .git that is a link is never opened"
        );

        let plain = root.path().join("plain");
        std::fs::create_dir_all(&plain).expect("a folder");
        let found = Workspace::of(&plain);
        assert_eq!((found.kind, found.top), (WorkspaceKind::Folder, plain));
    }
}
