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
        let owner = socket::bind_owner()?;
        self.serve(listener, owner, stop).await;
        if let Err(error) = std::fs::remove_file(self.dir.join(agent_threads::control::SOCKET)) {
            tracing::warn!(%error, "the socket of the runner was not removed");
        }
        self.save();
        Ok(())
    }

    /// The loop of the runner: the holder, the clients, and the stop.
    async fn serve(
        &mut self,
        listener: tokio::net::UnixListener,
        owner: Option<tokio::net::UnixListener>,
        stop: impl Future<Output = ()>,
    ) {
        let (sender, mut incoming) = mpsc::channel(256);
        let mut accepting = accept_connections(&self.dir, listener, owner, sender);
        tokio::pin!(stop);
        let mut look = tokio::time::interval(Duration::from_secs(10));
        let mut asked = self.asks.receiver.take().expect("one serve loop");
        let mut listed = self.model_asks.receiver.take().expect("one serve loop");
        let mut made = self.asks.made.take().expect("one serve loop");
        let mut created = self
            .project_creations
            .receiver
            .take()
            .expect("one serve loop");
        loop {
            tokio::select! {
                event = self.link.events.recv() => self.heard_holder(event).await,
                Some(call) = incoming.recv() => self.call(call).await,
                Some((id, answers)) = asked.recv() => self.asked(id, answers).await,
                result = completed::next(&mut listed, &mut created) => self.completed(result),
                Some(worktree) = made.recv() => self.worktree_made(worktree).await,
                _ = look.tick() => self.look_around().await,
                () = &mut stop => break,
            }
        }
        accepting.abort_all();
    }
}

fn accept_connections(
    dir: &std::path::Path,
    listener: tokio::net::UnixListener,
    owner: Option<tokio::net::UnixListener>,
    sender: mpsc::Sender<socket::Incoming>,
) -> tokio::task::JoinSet<()> {
    let mut tasks = tokio::task::JoinSet::new();
    // Managed workspaces have no local human client. Even a process outside
    // the holder ancestry receives only agent permissions on runner.sock.
    let local_authority = if owner.is_some() {
        socket::Authority::WorkspaceAgent
    } else {
        socket::Authority::Local
    };
    if let Some(listener) = owner {
        tasks.spawn(socket::accept(
            listener,
            sender.clone(),
            dir.to_owned(),
            socket::Authority::RootOwner,
        ));
    }
    tasks.spawn(socket::accept(
        listener,
        sender,
        dir.to_owned(),
        local_authority,
    ));
    tasks
}
