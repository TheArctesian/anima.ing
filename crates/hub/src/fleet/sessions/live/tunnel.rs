use tokio::sync::watch;

use super::Live;
use crate::{
    fleet::sessions::{end::Actor, model::EndReason},
    ids::SessionId,
};

/// `by` is the operator who called `endSession` (LOG-005), or `None` when the hub shuts down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EndCause {
    pub reason: EndReason,
    pub by: Option<Actor>,
}

/// A files relay's signal is never sent: shutdown only waits for it to close.
pub(super) type EndSignal = watch::Sender<Option<EndCause>>;

/// The tunnel task ends the session row before dropping this: `endSession` waits for the drop, then reads the row.
pub struct Tunnel {
    pub(super) session: SessionId,
    pub(super) cause: watch::Receiver<Option<EndCause>>,
    pub(super) live: Live,
}

impl Tunnel {
    /// Without waiting, for a tunnel task that can no longer await, such as one being dropped.
    pub(in crate::fleet::sessions) fn signalled(&self) -> Option<EndCause> {
        self.cause.borrow().clone()
    }

    pub(in crate::fleet::sessions) const fn live(&self) -> &Live {
        &self.live
    }

    /// Cancel safe. A closed channel means the hub's state is being torn down, which is a shutdown.
    pub async fn ended(&mut self) -> EndCause {
        loop {
            if let Some(cause) = self.cause.borrow_and_update().clone() {
                return cause;
            }
            if self.cause.changed().await.is_err() {
                return EndCause {
                    reason: EndReason::HubShutdown,
                    by: None,
                };
            }
        }
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        let mut state = self.live.state();
        if state
            .tunnels
            .get(&self.session)
            .is_some_and(|signal| self.cause.same_channel(&signal.subscribe()))
        {
            state.tunnels.remove(&self.session);
        }
    }
}

/// Held until the relay audits any transfer it cut short; shutdown waits for it, `endSession` does not.
pub(in crate::fleet::sessions) struct FilesRelay {
    pub(super) id: u64,
    pub(super) _held: watch::Receiver<Option<EndCause>>,
    pub(super) live: Live,
}

impl Drop for FilesRelay {
    fn drop(&mut self) {
        self.live.state().files_relays.remove(&self.id);
    }
}

pub(in crate::fleet::sessions) struct Closing(pub(super) Vec<EndSignal>);

impl Closing {
    /// Callers bound the wait.
    pub(in crate::fleet::sessions) async fn finished(self) {
        for signal in &self.0 {
            signal.closed().await;
        }
    }
}

pub(super) fn signal(end: &EndSignal, cause: EndCause) {
    end.send_if_modified(|current| {
        let first = current.is_none();
        if first {
            *current = Some(cause);
        }
        first
    });
}
