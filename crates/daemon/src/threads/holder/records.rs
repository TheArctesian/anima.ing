use super::{Shared, record, wire};
use record::Record;

impl Shared {
    /// Writes the record of the programs, so that a new holder ends what an old one left (H9).
    pub fn write_record(&self) {
        self.write_record_checked();
    }

    pub(super) fn write_record_checked(&self) -> bool {
        let running = self
            .threads
            .iter()
            .filter(|(_, slot)| slot.exit.is_none() || slot.left_behind)
            .map(|(thread, slot)| record::Entry {
                thread: thread.clone(),
                pid: slot.pid,
                started: slot.started.clone(),
                left: (slot.exit.is_some() && slot.left_behind).then(|| slot.seen.clone()),
            });
        let written = record::write(
            &self.dir,
            &Record {
                version: wire::VERSION,
                threads: running.chain(self.inherited.iter().cloned()).collect(),
            },
        );
        written
            .inspect_err(|error| {
                tracing::warn!(%error, "the record of the holder was not written");
            })
            .is_ok()
    }
}
