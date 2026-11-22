//! Where a thread works, as the details card of the client shows it (TH112): a Git checkout, a
//! linked worktree, or a folder with no Git, and its branch. The runner reads it at the start
//! and at the end of each turn (an agent may switch the branch), from the folder of the thread
//! upward, with no process of Git.

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
    /// The branch that `HEAD` names; none on a detached `HEAD`, with no Git, or when it does not
    /// read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

/// The most bytes of a branch name that the row says; a longer one is not a branch of a person.
const MOST_BRANCH_BYTES: usize = 255;

/// The most bytes read of a `.git` file (one line, `gitdir: <path>`) or of `HEAD`.
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
            // A link or any other thing that is not a plain file or folder is never opened.
            let gitdir = if found.is_dir() {
                Some(git)
            } else if found.is_file() {
                named_gitdir(&git, top)
            } else {
                None
            };
            let linked = found.is_file() && gitdir.as_deref().is_some_and(is_linked);
            return Self {
                kind: if linked {
                    WorkspaceKind::Worktree
                } else {
                    WorkspaceKind::Checkout
                },
                top: top.to_owned(),
                branch: gitdir.as_deref().and_then(branch_of),
            };
        }
        Self {
            kind: WorkspaceKind::Folder,
            top: folder.to_owned(),
            branch: None,
        }
    }
}

/// The start of a plain file, never a link, a pipe or a device, which could block a read.
fn start_of(file: &Path) -> Option<String> {
    if !std::fs::symlink_metadata(file).is_ok_and(|found| found.is_file()) {
        return None;
    }
    let mut text = String::new();
    std::fs::File::open(file)
        .and_then(|open| open.take(MOST_GIT_FILE_BYTES).read_to_string(&mut text))
        .ok()?;
    Some(text)
}

/// The folder of Git that a `.git` file names, from the folder that holds the file.
fn named_gitdir(git: &Path, top: &Path) -> Option<PathBuf> {
    let text = start_of(git)?;
    let named = text.lines().find_map(|line| line.strip_prefix("gitdir:"))?;
    Some(top.join(named.trim()))
}

/// A folder of Git below `worktrees/` of a repository is that of a linked worktree.
fn is_linked(gitdir: &Path) -> bool {
    gitdir
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name == "worktrees")
}

fn branch_of(gitdir: &Path) -> Option<String> {
    let head = start_of(&gitdir.join("HEAD"))?;
    let branch = head
        .lines()
        .next()?
        .strip_prefix("ref: refs/heads/")?
        .trim();
    let named = !branch.is_empty()
        && branch.len() <= MOST_BRANCH_BYTES
        && !branch.chars().any(char::is_control);
    named.then(|| branch.to_owned())
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
        std::fs::write(repo.join(".git/HEAD"), "ref: refs/heads/dev\n").expect("a HEAD");
        let found = Workspace::of(&repo.join("src/deep"));
        assert_eq!(
            (found.kind, found.top, found.branch.as_deref()),
            (WorkspaceKind::Checkout, repo.clone(), Some("dev"))
        );

        let side = root.path().join("side");
        std::fs::create_dir_all(&side).expect("a folder");
        let gitdir = repo.join(".git/worktrees/side");
        std::fs::write(side.join(".git"), format!("gitdir: {}\n", gitdir.display()))
            .expect("a .git file");
        std::fs::write(gitdir.join("HEAD"), "ref: refs/heads/fix/a\n").expect("a HEAD");
        let found = Workspace::of(&side);
        assert_eq!(
            (found.kind, found.branch.as_deref()),
            (WorkspaceKind::Worktree, Some("fix/a"))
        );
        std::fs::write(gitdir.join("HEAD"), "0123abcd\n").expect("a HEAD");
        assert_eq!(Workspace::of(&side).branch, None, "a detached HEAD");

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
        assert_eq!(
            (found.kind, found.top, found.branch),
            (WorkspaceKind::Folder, plain, None)
        );
    }
}
