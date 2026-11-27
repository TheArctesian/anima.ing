//! Authorizes local Git reads using the peer and holder's live process handles.

use std::sync::Arc;
use std::time::Duration;

use agent_threads::control::Outcome;
use agent_threads::git::read::{self, Request};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc};
use uuid::Uuid;

use super::{Runner, socket::Caller};
use crate::threads::git::Target;
use crate::threads::tree::identity::Process;
use crate::threads::wire::{Action, GitReadRoot};

mod ancestry;
use ancestry::Binding;

const PARALLEL_READS: usize = 8;
const READ_TIME: Duration = Duration::from_secs(20);

pub(super) struct Reads {
    slots: Arc<Semaphore>,
    send: mpsc::Sender<Completed>,
    pub(super) receive: Option<mpsc::Receiver<Completed>>,
}

impl Default for Reads {
    fn default() -> Self {
        let (send, receive) = mpsc::channel(PARALLEL_READS);
        Self {
            slots: Arc::new(Semaphore::new(PARALLEL_READS)),
            send,
            receive: Some(receive),
        }
    }
}

struct Pending {
    back: Caller,
    request: Request,
    _slot: OwnedSemaphorePermit,
}

pub(super) struct Completed {
    pending: Pending,
    result: Finished,
}

enum Finished {
    Bound(Option<Binding>),
    Summary(Binding, read::Outcome),
}

impl Runner {
    pub(super) fn git_read(&self, back: Caller, peer: Option<Process>, request: Request) {
        let (Some(roots), Some(peer)) = (self.link.git_roots(), peer) else {
            refuse(&back, read::Refusal::Unsupported);
            return;
        };
        let Ok(slot) = self.git_reads.slots.clone().try_acquire_owned() else {
            refuse(&back, read::Refusal::Unavailable);
            return;
        };
        let pending = Pending {
            back,
            request,
            _slot: slot,
        };
        let send = self.git_reads.send.clone();
        tokio::spawn(async move {
            let proof = tokio::task::spawn_blocking(move || Binding::find(roots, peer)).await;
            let result = Finished::Bound(proof.ok().flatten());
            let _ = send.send(Completed { pending, result }).await;
        });
    }

    pub(super) fn git_read_completed(&self, completed: Completed) {
        let Completed { pending, result } = completed;
        match result {
            Finished::Bound(Some(binding)) => self.summarize(pending, binding),
            Finished::Bound(None) => refuse(&pending.back, read::Refusal::Unbound),
            Finished::Summary(binding, result) => self.deliver(&pending, &binding, result),
        }
    }

    fn summarize(&self, pending: Pending, binding: Binding) {
        if self.with_binding(&pending, &binding, || ()).is_none() {
            return;
        }
        if !self.registered_worktree(&pending, &binding) {
            self.deliver(
                &pending,
                &binding,
                read::Outcome::Refused {
                    why: read::Refusal::NoWorktree,
                },
            );
            return;
        }
        let Some(engine) = self.git.read_engine() else {
            refuse(&pending.back, read::Refusal::Unavailable);
            return;
        };
        let send = self.git_reads.send.clone();
        tokio::spawn(async move {
            let target = Target {
                thread: binding.thread,
                repository: &pending.request.repository,
            };
            let read = engine.read_summary(target, pending.request.read);
            let outcome =
                tokio::time::timeout(READ_TIME, read)
                    .await
                    .unwrap_or(read::Outcome::Refused {
                        why: read::Refusal::Unavailable,
                    });
            let result = Finished::Summary(binding, outcome);
            let _ = send.send(Completed { pending, result }).await;
        });
    }

    fn deliver(&self, pending: &Pending, binding: &Binding, result: read::Outcome) {
        let success = !matches!(result, read::Outcome::Refused { .. });
        if success && !self.registered_worktree(pending, binding) {
            return;
        }
        self.with_binding(pending, binding, || {
            pending.back.answer(Outcome::GitRead { result });
        });
    }

    fn with_binding<T>(
        &self,
        pending: &Pending,
        binding: &Binding,
        run: impl FnOnce() -> T,
    ) -> Option<T> {
        if !self.running_git_root(binding.thread, &binding.root) {
            return None;
        }
        self.link
            .with_git_root(&binding.roots, &binding.root, || {
                pending
                    .back
                    .client
                    .with_open(|| binding.live().then(run))
                    .flatten()
            })
            .flatten()
    }

    fn running_git_root(&self, thread: Uuid, root: &GitReadRoot) -> bool {
        self.holder.fits
            && self.threads.get(&thread).is_some_and(|thread| {
                thread.row.state != agent_threads::control::RunState::Stopped
            })
            && !self.unanswered.values().any(|(command, _)| {
                matches!(&command.action, Action::Stop { thread } if thread == &root.thread)
            })
    }

    fn registered_worktree(&self, pending: &Pending, binding: &Binding) -> bool {
        let repository = &pending.request.repository;
        let Some(thread) = self.threads.get(&binding.thread) else {
            return false;
        };
        let Some(registered) = self.git.repository_of(binding.thread, &thread.row.folder) else {
            return false;
        };
        repository == &registered
            && self.git.read_engine().is_some_and(|engine| {
                engine.registered(Target {
                    thread: binding.thread,
                    repository,
                })
            })
    }
}

fn refuse(back: &Caller, why: read::Refusal) {
    back.client.with_open(|| {
        back.answer(Outcome::GitRead {
            result: read::Outcome::Refused { why },
        });
    });
}

#[cfg(all(test, target_os = "linux", target_pointer_width = "64"))]
mod tests;
