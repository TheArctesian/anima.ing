//! Carries the account id only; the subscription reads the account as it is then.

use tokio::sync::broadcast;

use crate::ids::SourceAccountId;

const CAPACITY: usize = 64;

#[derive(Clone)]
pub struct Feed {
    sender: broadcast::Sender<SourceAccountId>,
}

impl Feed {
    #[must_use]
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(CAPACITY);
        Self { sender }
    }

    pub fn publish(&self, account: SourceAccountId) {
        // `send` fails only when nobody is subscribed: there is nobody to tell.
        let _ = self.sender.send(account);
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<SourceAccountId> {
        self.sender.subscribe()
    }
}

impl Default for Feed {
    fn default() -> Self {
        Self::new()
    }
}
