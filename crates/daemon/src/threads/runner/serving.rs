//! The lifetime of the runner socket and its event loop.

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
        self.project_creations.shutdown().await;
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
        let mut look = tokio::time::interval(Duration::from_secs(10));
        let mut asked = self.asks.receiver.take().expect("one serve loop");
        let mut listed = self.model_asks.receiver.take().expect("one serve loop");
        let mut made = self.asks.made.take().expect("one serve loop");
        loop {
            tokio::select! {
                event = self.link.events.recv() => self.heard_holder(event).await,
                Some(call) = incoming.recv() => self.call(call).await,
                Some((id, answers)) = asked.recv() => self.asked(id, answers).await,
                result = completed::next(&mut listed, &mut self.project_creations) => self.completed(result),
                Some(worktree) = made.recv() => self.worktree_made(worktree).await,
                _ = look.tick() => self.look_around().await,
                () = &mut stop => break,
            }
        }
        accepting.abort();
    }
}
