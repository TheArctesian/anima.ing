//! The complete announcements and buffered points for one push.

use super::{Pusher, VERSION};
use daemon_core::wire::{Announcements, Batch};

impl Pusher {
    pub(super) fn batch(&mut self, count: usize) -> Batch {
        let announced = Announcements {
            agent_usage: self.usage.outgoing(),
            #[cfg(unix)]
            agent_threads: self.thread_summaries.outgoing(self.marks.as_ref()),
            ..self.announced.clone()
        };
        let mut batch = Batch::new(
            self.device,
            self.backlog.batch().first(count).points(),
            announced,
        )
        .from_daemon(VERSION);
        if let Some(keys) = &self.keys {
            batch = batch.with_keys(keys.held());
        }
        batch
    }
}
