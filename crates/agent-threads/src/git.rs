//! The Git of a thread (stage 7 of the copy, tasks TH41 to TH45): what a client asks of the
//! runner and what it gets back. The page, the `anima` command and the runner share these frames.
//!
//! Pure: only the frames. The runner does the work (`anima-daemon`, `threads::git`).

use serde::{Deserialize, Serialize};

/// What a client asks about the worktree of one repository of a thread.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "git", rename_all = "snake_case")]
pub enum GitAction {
    /// The branch, what changed, and how the branch stands to its remote.
    Status,
    /// The checkpoints of the thread, the oldest first.
    Checkpoints,
    /// The changed files with their patches.
    Changes {
        scope: Scope,
        #[serde(default)]
        ignore_whitespace: bool,
    },
    /// Puts the files back as they were at the checkpoint of `turn`. For the person only.
    Revert { turn: u32 },
    /// Commits all files, or the chosen ones, as they are in the snapshot that the person
    /// looked at. For the person only.
    Commit {
        message: String,
        /// The `snapshot` of the [`Changes`] of [`Scope::Uncommitted`] that the page showed:
        /// the commit holds these files and no later edit.
        reviewed: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        files: Option<Vec<String>>,
        /// "Commit on new branch": the branch to make and check out first.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        new_branch: Option<String>,
    },
    /// Pushes the branch, never by force. For the person only.
    Push {
        /// The person said yes to a push of the default branch.
        #[serde(default)]
        default_branch_allowed: bool,
    },
    /// Checks the branch and gives the page of the forge where the person opens the pull
    /// request. For the person only.
    PullRequest,
    /// Saves a private ref and retires the original directory. For the person only.
    Remove,
    /// Removes the checkpoints of the turns before `before`, to free the disk that they use.
    /// The files of the worktree and the later checkpoints stay. For the person only.
    Forget { before: u32 },
    /// The person read what changed in who a commit is from and where a push goes
    /// ([`Status::changed`]) and accepts it for a commit and a push. For the person only.
    Trust {
        /// The [`Status::accept`] of the status that the page showed: what the person read.
        /// The runner refuses when the list is not this one any more.
        shown: String,
    },
}

impl GitAction {
    /// The action changes files, history or a remote, so only the person can ask for it: a
    /// call from a process of an agent is refused (THR-049).
    #[must_use]
    pub const fn for_the_person(&self) -> bool {
        !matches!(
            self,
            Self::Status | Self::Checkpoints | Self::Changes { .. }
        )
    }

    /// The action is refused while a turn of the thread runs: the files change then.
    #[must_use]
    pub const fn waits_for_the_turn(&self) -> bool {
        matches!(
            self,
            Self::Revert { .. }
                | Self::Remove
                | Self::Forget { .. }
                | Self::Commit { .. }
                | Self::Push { .. }
                | Self::Trust { .. }
        )
    }
}

/// Which changes to show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum Scope {
    /// The files as they are now against the last commit ("Uncommitted").
    Uncommitted,
    /// One turn: its checkpoint against the one before it.
    Turn { turn: u32 },
    /// The whole thread: the first checkpoint against the last one ("Changes").
    Thread,
}

/// The answer of the runner.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum GitOutcome {
    Status(Status),
    Checkpoints {
        checkpoints: Vec<Checkpoint>,
    },
    Changes(Changes),
    Reverted {
        turn: u32,
        undo: String,
    },
    Committed {
        commit: String,
        branch: Option<String>,
    },
    Pushed {
        remote: String,
        /// Where the push went, with each credential removed.
        url: String,
        branch: String,
        up_to_date: bool,
    },
    PullRequest(PullRequestPage),
    Removed {
        kept: Option<String>,
        /// The complete retained directory, when the runner supports retirement.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        recovery: Option<std::path::PathBuf>,
    },
    Trusted,
    Refused {
        why: GitRefusal,
    },
}

/// Why the runner did not do an action. `Failed` holds the words of `git`, with each credential
/// removed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum GitRefusal {
    /// An agent asked for an action that is for the person.
    FromAnAgent,
    /// The thread has no worktree of the runner, so its files are never changed.
    NoWorktree,
    /// A path or a name leaves its folder or has a character that is not allowed.
    BadPath,
    /// A turn runs: "Interrupt the current turn before reverting checkpoints."
    Running,
    UnknownCheckpoint,
    NothingToCommit,
    EmptyMessage,
    DetachedHead,
    NoRemote,
    /// The branch is behind its upstream: pull or rebase first.
    Behind,
    /// The branch is the default branch and the person did not say yes to it.
    DefaultBranch,
    /// The runner did not make this branch for the thread, so it does not move it: commit on
    /// a new branch.
    NotOurBranch,
    /// The branch has commits that are not pushed.
    NotPushed,
    NoCommits,
    /// The forge of the remote is not one the runner can make a page for.
    UnknownForge,
    BranchExists,
    /// The branch or commit to start a worktree from does not exist.
    UnknownBase,
    NotARepository,
    GitNotFound,
    /// Who a commit is from or where a push goes changed in the repository since the person
    /// last accepted them: `what` names each one. A commit or a push would take them.
    Changed {
        what: Vec<String>,
    },
    /// The snapshot to commit is not one of the files as the person saw them on this branch:
    /// the branch moved, or the snapshot is unknown. The page reads the changes again.
    NotReviewed,
    /// A thread of this computer runs in a folder that holds the records of Anima's Git, so
    /// its agent can write them. No Git action of any thread runs until that thread ends.
    RecordsHeld,
    /// The worktree of the thread is there, but the system removed its checkpoints and its
    /// record after a long time without use. The files of the worktree are as they were.
    Expired,
    /// Worktrees, checkpoints and the other Git actions of a thread are not built for Windows
    /// yet (ruling of the operator).
    NotOnWindows,
    Failed {
        said: String,
    },
}

/// What the runner does while it makes the worktree of a thread. The first worktree of a large
/// repository takes long: the page shows the step, and the person can cancel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preparing {
    /// The history of the repository is copied and checked. Once for each project.
    History,
    /// The files of the worktree are written.
    Files,
}

/// How a branch stands, as the details card shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Status {
    /// `None` on a detached HEAD.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_branch: Option<String>,
    pub is_default_branch: bool,
    pub has_remote: bool,
    /// Where a push goes, with each credential removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub push_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// Commits of the branch that the default branch does not have.
    pub ahead_of_default: u32,
    pub files: Vec<ChangedFile>,
    pub additions: u32,
    pub deletions: u32,
    /// Who a commit is from (when the repository sets it) and where a push goes, each as a
    /// line to read, when it is not as the person last accepted it; a commit or a push is
    /// refused until [`GitAction::Trust`]. No hook of the repository runs for either.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changed: Vec<String>,
    /// A digest of those lines as they are now, which [`GitAction::Trust`] names.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub accept: String,
}

/// One checkpoint: the files of the worktree at the end of a turn (0 is the start).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub turn: u32,
    pub commit: String,
    /// Seconds since 1970.
    pub at: i64,
}

/// The changed files of a scope.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Changes {
    pub files: Vec<ChangedFile>,
    pub additions: u32,
    pub deletions: u32,
    /// The complete patches could not be shown. The counts are still available.
    pub truncated: bool,
    /// For a person's complete, unfiltered [`Scope::Uncommitted`] view: the snapshot
    /// that [`GitAction::Commit`] names as `reviewed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
}

/// How a file changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Deleted,
    #[default]
    Modified,
    Renamed,
}

/// One changed file with its counts and, for a diff, its patch.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    pub kind: ChangeKind,
    pub additions: u32,
    pub deletions: u32,
    #[serde(default)]
    pub binary: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patch: Option<String>,
}

/// The page of the forge where the person opens the pull request, with what it compares.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestPage {
    pub url: String,
    pub base: String,
    pub head: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_action_that_changes_something_is_for_the_person() {
        let reads = [
            GitAction::Status,
            GitAction::Checkpoints,
            GitAction::Changes {
                scope: Scope::Thread,
                ignore_whitespace: true,
            },
        ];
        let changes = [
            GitAction::Revert { turn: 1 },
            GitAction::Commit {
                message: "m".to_owned(),
                reviewed: "0".repeat(40),
                files: None,
                new_branch: None,
            },
            GitAction::Push {
                default_branch_allowed: false,
            },
            GitAction::PullRequest,
            GitAction::Remove,
            GitAction::Trust {
                shown: String::new(),
            },
        ];
        assert!(reads.iter().all(|action| !action.for_the_person()));
        assert!(changes.iter().all(GitAction::for_the_person));
    }

    #[test]
    fn a_push_frame_without_the_yes_does_not_allow_the_default_branch() {
        let action: GitAction = serde_json::from_str(r#"{"git":"push"}"#).expect("a push");
        assert_eq!(
            action,
            GitAction::Push {
                default_branch_allowed: false
            }
        );
    }
}
