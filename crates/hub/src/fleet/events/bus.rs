//! In-process broadcast of new events and status changes for subscriptions (LOG-010).

use chrono::{DateTime, Utc};
use tokio::sync::broadcast;

use super::model::{EventSummary, HeartbeatSummary};
use crate::{
    ids::{CheckId, DeviceId},
    status::Status,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signal {
    Event(EventSummary),
    DeviceStatus {
        device: DeviceId,
        status: Status,
        at: DateTime<Utc>,
    },
    Heartbeat {
        check: CheckId,
        device: DeviceId,
        beat: HeartbeatSummary,
    },
}

/// A slow subscriber skips the oldest signals; publishers never wait.
#[derive(Clone, Debug)]
pub struct Bus {
    sender: broadcast::Sender<Signal>,
}

impl Bus {
    /// # Panics
    /// If `capacity` is 0.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn publish(&self, signal: Signal) {
        // `send` fails only when nobody is subscribed; a live update with no listener has nothing to report.
        let _ = self.sender.send(signal);
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<Signal> {
        self.sender.subscribe()
    }
}
