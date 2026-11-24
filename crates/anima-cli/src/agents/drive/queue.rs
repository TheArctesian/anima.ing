//! `anima agents queue` (TH109). Only the person changes the queue.

use agent_threads::control::Action;
use uuid::Uuid;

use super::simple;
use crate::failure::Failure;
use crate::invocation::Invocation;

#[derive(Debug, clap::Args)]
pub struct QueueArgs {
    thread: Uuid,
    /// The prompt that waits; with `--replace`, its new words.
    #[arg(required_unless_present_any = ["drop", "send"])]
    text: Option<String>,
    /// Give the queued prompt with this ID new words, as `anima agents running --json` shows it.
    #[arg(long, requires = "text", conflicts_with_all = ["drop", "send"])]
    replace: Option<Uuid>,
    /// Take the queued prompt with this ID out of the queue.
    #[arg(long, conflicts_with_all = ["text", "send"])]
    drop: Option<Uuid>,
    /// Send the queued prompt with this ID now: into the running turn, or as the next turn.
    #[arg(long, conflicts_with = "text")]
    send: Option<Uuid>,
}

impl QueueArgs {
    /// # Errors
    /// [`Failure`] for a refusal of the runner.
    pub async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let thread = self.thread;
        let (action, done) = match (self.text, self.replace, self.drop, self.send) {
            (Some(text), Some(id), _, _) => (
                Action::Requeue {
                    thread,
                    queued: id,
                    text,
                },
                "Changed.",
            ),
            (_, _, Some(id), _) => (
                Action::Unqueue { thread, queued: id },
                "Taken out of the queue.",
            ),
            (_, _, _, Some(id)) => (Action::SendQueued { thread, queued: id }, "Sent."),
            (text, ..) => (
                Action::Queue {
                    thread,
                    text: text.unwrap_or_default(),
                },
                "Queued. It goes at once when the thread waits for a prompt.",
            ),
        };
        simple(invocation, action, done).await
    }
}
