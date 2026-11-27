use super::Shared;
use crate::threads::tree::identity::Process;
use crate::threads::wire::{Event, GitReadRoot, ThreadKey};

#[derive(Default)]
pub(super) struct GitReads {
    pub(super) negotiated: bool,
    generation: u64,
}

pub(super) struct Root {
    process: Process,
    generation: u64,
}

impl Root {
    fn frame(&self, thread: &ThreadKey) -> GitReadRoot {
        GitReadRoot {
            thread: thread.clone(),
            process: self.process.identity(),
            generation: self.generation,
        }
    }
}

impl Shared {
    pub(in crate::threads) fn git_read_hello(&mut self, asked: bool) -> Option<Vec<GitReadRoot>> {
        self.git_reads.negotiated = asked && !self.draining;
        self.git_reads.negotiated.then(|| {
            self.threads
                .iter()
                .filter_map(|(thread, slot)| {
                    let root = slot.git_root.as_ref()?;
                    (!slot.reaped && slot.exit.is_none() && root.process.live())
                        .then(|| root.frame(thread))
                })
                .collect()
        })
    }

    /// Pin the owned child before it is handed to the task that can reap it.
    pub(in crate::threads) fn enroll_git_read(&mut self, thread: &ThreadKey, pid: u32) {
        let Some(process) = Process::open(pid) else {
            return;
        };
        let Some(generation) = self.git_reads.generation.checked_add(1) else {
            return;
        };
        let Some(slot) = self.threads.get_mut(thread) else {
            return;
        };
        self.git_reads.generation = generation;
        let root = Root {
            process,
            generation,
        };
        let frame = root.frame(thread);
        slot.git_root = Some(root);
        self.git_read_event(frame, true);
    }

    pub(in crate::threads) fn revoke_git_read(&mut self, thread: &ThreadKey) {
        if let Some(root) = self
            .threads
            .get_mut(thread)
            .and_then(|slot| slot.git_root.take())
        {
            self.git_read_event(root.frame(thread), false);
        }
    }

    fn git_read_event(&self, root: GitReadRoot, enrolled: bool) {
        if self.git_reads.negotiated {
            self.send(&Event::GitReadRoot { root, enrolled });
        }
    }
}
