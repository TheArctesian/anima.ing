//! A new link to the holder after the old one closed (THR-113, H10).

use super::super::Runner;
use crate::threads::link;
use crate::threads::wire::Command;

impl Runner {
    /// A new link to the holder. The commands that the holder did not answer go again with
    /// their own IDs, so that a line the holder already wrote is not written twice and each call
    /// that waits gets its answer; a start waits for the holder to say what it holds.
    pub(in crate::threads::runner) async fn reconnect(&mut self) {
        tracing::warn!("the link to the holder closed; connecting again");
        self.holder.fits = false;
        self.remote.disconnect();
        self.holder.newer = false;
        // Until the next hello nothing is known of the holder: no Quit goes to a new one.
        self.holder.older = false;
        let next = self.link.next_id();
        self.link = link::reach_patiently(&self.program, &self.dir).await;
        self.link.go_on_from(next);
        self.send_again().await;
    }

    /// Sends each command that the holder did not answer again, in the order it went first.
    async fn send_again(&mut self) {
        let again: Vec<Command> = self.unanswered.values().cloned().collect();
        for command in again {
            if let Err(error) = self.link.resend(&command).await {
                tracing::warn!(%error, "the holder did not take a command sent again");
                return;
            }
        }
    }
}
