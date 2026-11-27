//! The lifetime of the runner socket and its event loop.

use std::time::Duration;

use agent_threads::drive::Program;
use tokio::sync::mpsc;
use tokio::time::Interval;

use super::{Runner, RunnerError, completed, models, silence, socket, start};
use crate::threads::wire;

enum Ready {
    Holder(Option<wire::Event>),
    Call(socket::Incoming),
    Answers(u64, silence::Answers),
    Completed(completed::Completed),
    Worktree(Box<start::WorktreeMade>),
    Look,
}

struct Sources {
    incoming: mpsc::Receiver<socket::Incoming>,
    asked: mpsc::Receiver<(u64, silence::Answers)>,
    listed: mpsc::Receiver<(Program, models::Listed)>,
    made: mpsc::Receiver<start::WorktreeMade>,
    look: Interval,
}

impl Sources {
    async fn next(&mut self, runner: &mut Runner) -> Ready {
        tokio::select! {
            event = runner.link.events.recv() => Ready::Holder(event),
            Some(call) = self.incoming.recv() => Ready::Call(call),
            Some((id, answers)) = self.asked.recv() => Ready::Answers(id, answers),
            result = completed::next(&mut self.listed, &mut runner.project_creations) => Ready::Completed(result),
            Some(worktree) = self.made.recv() => Ready::Worktree(Box::new(worktree)),
            _ = self.look.tick() => Ready::Look,
        }
    }
}

impl Runner {
    pub(super) async fn listen(
        &mut self,
        stop: impl Future<Output = ()>,
    ) -> Result<(), RunnerError> {
        let listener = socket::bind(&self.dir)?;
        let owner = socket::bind_owner()?;
        self.serve(listener, owner, stop).await;
        self.project_creations.shutdown().await;
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
        let (sender, incoming) = mpsc::channel(256);
        let mut accepting = accept_connections(&self.dir, listener, owner, sender);
        tokio::pin!(stop);
        let mut sources = Sources {
            incoming,
            asked: self.asks.receiver.take().expect("one serve loop"),
            listed: self.model_asks.receiver.take().expect("one serve loop"),
            made: self.asks.made.take().expect("one serve loop"),
            look: tokio::time::interval(Duration::from_secs(10)),
        };
        loop {
            let ready = tokio::select! {
                biased;
                () = &mut stop => break,
                ready = sources.next(self) => ready,
            };
            // Stop is checked before taking another event. Once taken, a handler finishes.
            match ready {
                Ready::Holder(event) => self.heard_holder(event).await,
                Ready::Call(call) => self.call(call).await,
                Ready::Answers(id, answers) => self.asked(id, answers).await,
                Ready::Completed(result) => self.completed(result),
                Ready::Worktree(worktree) => self.worktree_made(*worktree).await,
                Ready::Look => self.look_around().await,
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
#[cfg(test)]
mod tests;
