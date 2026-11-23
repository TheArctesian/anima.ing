//! The live feed of collections, for the `metricsAdded` subscription (D-9, D-15).

use tokio::sync::broadcast;

use super::MetricsAdded;

const CAPACITY: usize = 256;

/// A slow subscriber skips the oldest; the collector never waits for a subscriber.
#[derive(Clone, Debug)]
pub struct Feed {
    sender: broadcast::Sender<MetricsAdded>,
}

impl Feed {
    #[must_use]
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(CAPACITY);
        Self { sender }
    }

    pub fn publish(&self, added: MetricsAdded) {
        // `send` fails only when nobody is subscribed, and then there is nothing to report.
        let _ = self.sender.send(added);
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<MetricsAdded> {
        self.sender.subscribe()
    }
}

impl Default for Feed {
    fn default() -> Self {
        Self::new()
    }
}
