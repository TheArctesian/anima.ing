//! The live broadcast of job changes, for the `buildJobUpdated` subscription (BLD-081).

use tokio::sync::broadcast;

use super::JobUpdate;

const CAPACITY: usize = 256;

/// A full broadcast holds at most 16 MiB of lines; a subscriber that sees a gap reads `buildJobLog`.
pub(super) const MAX_UPDATE_LINE_BYTES: usize = 64 * 1024;

pub(super) fn bounded(mut update: JobUpdate) -> JobUpdate {
    let mut total = 0;
    let fitting = update
        .lines
        .iter()
        .rev()
        .take_while(|line| {
            total += line.text.len();
            total <= MAX_UPDATE_LINE_BYTES
        })
        .count();
    update.lines.drain(..update.lines.len() - fitting);
    update
}

/// A slow subscriber skips the oldest; a writer never waits for a subscriber.
#[derive(Clone, Debug)]
pub struct Updates {
    sender: broadcast::Sender<JobUpdate>,
}

impl Updates {
    #[must_use]
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(CAPACITY);
        Self { sender }
    }

    pub(super) fn publish(&self, update: JobUpdate) {
        // `send` fails only when nobody is subscribed, and then there is nothing to report.
        let _ = self.sender.send(bounded(update));
    }

    pub(super) fn subscribe(&self) -> broadcast::Receiver<JobUpdate> {
        self.sender.subscribe()
    }
}

impl Default for Updates {
    fn default() -> Self {
        Self::new()
    }
}
