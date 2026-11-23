//! A new thread in a worktree of its own (TC7 step W1, TH116): the person chose "New worktree"
//! for a repository. The runner makes the worktree away from the loop (the first one of a
//! project copies its history, which takes long), then starts the program in it. The launcher
//! knows the thread from the moment the worktree is decided, because the copy reads the
//! repository with the rights of the thread.
//!
//! I/O: starts the task that makes the worktree.

use agent_threads::control::Refusal;
use agent_threads::git::GitRefusal;
use uuid::Uuid;

use super::super::launcher::Seat;
use super::super::launcher::read::Confinement;
use super::{Asking, Runner};
use crate::threads::git::worktree::{Made, Plan};

/// What the task that makes a worktree answers.
pub(in crate::threads::runner) type WorktreeMade = (Uuid, Result<Made, GitRefusal>);

impl Runner {
    /// Decides the thread of a start in a new worktree and makes its worktree in a task; the
    /// start goes on in [`Runner::worktree_made`].
    pub(super) fn make_worktree(&mut self, mut asking: Asking) -> Result<(), Refusal> {
        let Some(base) = asking.new.worktree.clone().map(|worktree| worktree.base) else {
            return Err(Refusal::BadValue);
        };
        // A worktree is of a repository: a start with no project has none.
        if asking.new.folder.as_os_str().is_empty() {
            return Err(Refusal::BadValue);
        }
        let worktree = |why| Refusal::Worktree { why };
        let engine = self.git.engine().map_err(worktree)?.clone();
        let id = Uuid::now_v7();
        let Some(folder) = self.git.thread_folder(id) else {
            return Err(worktree(GitRefusal::BadPath));
        };
        let tmp = super::private_tmp::made(&self.dir, id).map_err(|error| {
            tracing::warn!(%error, %id, "the temporary folder of a thread could not be made");
            Refusal::FolderNotPermitted
        })?;
        let daemon = self.dir.parent().unwrap_or(&self.dir);
        let confinement = Confinement {
            mode: asking.new.mode,
            program: asking.new.program,
            codex: Some(asking.path.clone()),
            kept: crate::threads::notice::held_anima(&folder, daemon),
        };
        {
            let mut rights = self
                .rights
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            rights
                .coming
                .insert(id, Seat::new(folder, confinement, tmp));
            rights.copies.insert(id, asking.path.clone());
        }
        let project = asking.new.folder.clone();
        // The launcher keeps a coming seat only while its start is in `making`.
        asking.id = Some(id);
        self.asks.making.insert(id, asking);
        self.rights_now();
        let sender = self.asks.worktrees.clone();
        tokio::spawn(async move {
            let plan = Plan {
                thread: id,
                project: &project,
                base: base.as_deref(),
            };
            let made = engine.worktree(plan).await;
            // A runner that ended does not need the answer.
            let _ = sender.send((id, made)).await;
        });
        Ok(())
    }

    /// The worktree of the thread `id` is made, or not: the start goes on in it, or is refused
    /// with the answer of the engine.
    pub(in crate::threads::runner) async fn worktree_made(&mut self, (id, made): WorktreeMade) {
        let Some(mut asking) = self.asks.making.remove(&id) else {
            return;
        };
        let back = asking.back.clone();
        let started = match made {
            Ok(made) => {
                asking.new.folder = made.worktree.folder().to_owned();
                self.start_in(asking).await
            }
            Err(why) => Err(Refusal::Worktree { why }),
        };
        if let Err(why) = started {
            {
                let mut rights = self
                    .rights
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                rights.coming.remove(&id);
            }
            self.rights_now();
            self.note(
                super::super::log::Kind::Refusal,
                Some(id),
                super::super::calls::refusal_word(&why),
            );
            back.answer(super::super::holder_events::refused(why));
        }
    }
}
