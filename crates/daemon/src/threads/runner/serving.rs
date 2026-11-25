//! The lifetime of the runner socket and its event loop.

use agent_threads::control::{Action, Call};

use super::{Runner, RunnerError, completed, socket};
use std::time::Duration;
use tokio::sync::mpsc;

impl Runner {
    pub(super) async fn listen(
        &mut self,
        stop: impl Future<Output = ()>,
    ) -> Result<(), RunnerError> {
        let listener = socket::bind(&self.dir)?;
        self.serve(listener, stop).await;
        if let Err(error) = std::fs::remove_file(self.dir.join(agent_threads::control::SOCKET)) {
            tracing::warn!(%error, "the socket of the runner was not removed");
        }
        self.save();
        Ok(())
    }

    /// The loop of the runner: the holder, the clients, and the stop.
    async fn serve(&mut self, listener: tokio::net::UnixListener, stop: impl Future<Output = ()>) {
        let (sender, mut incoming) = mpsc::channel(256);
        let accepting = tokio::spawn(socket::accept(listener, sender, self.dir.clone()));
        tokio::pin!(stop);
        let mut remote = self.remote.open();
        let mut remote_tick = tokio::time::interval(Duration::from_secs(10));
        remote_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut look = tokio::time::interval(Duration::from_secs(10));
        let mut asked = self.asks.receiver.take().expect("one serve loop");
        let mut listed = self.model_asks.receiver.take().expect("one serve loop");
        let mut made = self.asks.made.take().expect("one serve loop");
        let mut git_reads = self.git_reads.receive.take().expect("one serve loop");
        let mut created = self
            .project_creations
            .receiver
            .take()
            .expect("one serve loop");
        loop {
            tokio::select! {
                event = self.link.events.recv() => self.heard_holder(event).await,
                Some(call) = incoming.recv() => self.incoming(call).await,
                Some((id, answers)) = asked.recv() => self.asked(id, answers).await,
                result = completed::next(&mut listed, &mut created, &mut git_reads) => self.completed(result),
                Some(worktree) = made.recv() => self.worktree_made(worktree).await,
                Some(event) = super::remote_code::next(&mut remote, &mut look, &mut remote_tick) => self.remote_event(event).await,
                () = &mut stop => break,
            }
        }
        accepting.abort();
    }

    async fn incoming(&mut self, incoming: socket::Incoming) {
        match incoming {
            socket::Incoming {
                call:
                    Call {
                        id,
                        action: Action::GitRead(request),
                    },
                client,
                git_peer,
                ..
            } => {
                self.git_read(client.reply_to(id), git_peer, request);
            }
            other => self.call(other).await,
        }
    }
}
