//! What a call that makes a worktree made so far, and how it goes again: when the call fails or
//! is cancelled, and when the runner refuses the start of the thread before an agent ran in the
//! new worktree ([`Pending`]).
//!
//! I/O: removes folders and files that the call made; nothing else.

use std::path::PathBuf;

use tokio::sync::OwnedMutexGuard;
use uuid::Uuid;

use super::super::place::Home;
use super::super::project::{Layout, linked, refs};
use super::Made;

/// What a call that makes a worktree made so far. When it is dropped before [`Undo::keep`]
/// (a step failed, the call was cancelled, or the start was refused), each of them goes again:
/// they hold no work of a thread.
#[derive(Debug)]
pub(in super::super) struct Undo {
    pub(super) home: Home,
    pub(super) thread: Uuid,
    pub(super) repository: String,
    /// The Git folder of the runner for the worktree.
    pub(super) own: PathBuf,
    /// The folder of the worktree. It was not there when the call started: what is there now
    /// is of this call.
    pub(super) folder: PathBuf,
    /// The entry of the worktree in the Git folder of the person.
    pub(super) entry: Option<Layout>,
    /// The branch that this call made: the shared Git folder and the name of its ref. It is
    /// taken back only while it still names [`Undo::at`].
    pub(super) branch: Option<(PathBuf, String)>,
    /// The commit that the branch starts at.
    pub(super) at: String,
    pub(super) kept: bool,
}

impl Undo {
    /// The worktree is the thread's now: nothing goes again.
    pub(super) fn keep(mut self) {
        self.kept = true;
    }
}

impl Drop for Undo {
    fn drop(&mut self) {
        if self.kept {
            return;
        }
        let _ = std::fs::remove_dir_all(&self.own);
        self.home.forget(self.thread, &self.repository);
        let _ = std::fs::remove_dir_all(&self.folder);
        if let Some(entry) = &self.entry {
            let _ = linked::remove(entry);
        }
        if let Some((common, full)) = &self.branch {
            let _ = refs::take_back(common, full, &self.at);
        }
        // The folder of the thread goes when it is empty; one with a second repository stays.
        let _ = self.folder.parent().map(std::fs::remove_dir);
    }
}

/// A worktree that is made, before the start of its thread. The runner calls
/// [`Pending::keep`] when the start passed, before an agent runs in it. Dropped without that,
/// what the call made goes again and nothing is saved (THR-097: it holds no work yet); a
/// worktree that was there already ([`Made::reused`]) stays.
///
/// While it is there, no other worktree of its thread is made. The undo runs on a blocking
/// thread of the tokio runtime of the drop, so a drop never waits for the file system; with no
/// runtime, it runs at once in the drop. A runtime that shuts down before the undo ran can leave
/// the worktree; the person removes it then as each other one.
#[derive(Debug)]
pub struct Pending {
    made: Made,
    undo: Option<Undo>,
    alone: Option<OwnedMutexGuard<()>>,
}

impl Pending {
    pub(in super::super) fn new(
        made: Made,
        undo: Option<Undo>,
        alone: OwnedMutexGuard<()>,
    ) -> Self {
        Self {
            made,
            undo,
            alone: Some(alone),
        }
    }

    /// The worktree, its folder and its branch, to read before the start.
    #[must_use]
    pub const fn made(&self) -> &Made {
        &self.made
    }

    /// The start passed: the worktree is the thread's.
    #[must_use]
    pub fn keep(mut self) -> Made {
        if let Some(undo) = self.undo.take() {
            undo.keep();
        }
        self.made.clone()
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        let Some(undo) = self.undo.take() else {
            return;
        };
        let alone = self.alone.take();
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => {
                runtime.spawn_blocking(move || drop((undo, alone)));
            }
            Err(_) => drop((undo, alone)),
        }
    }
}
