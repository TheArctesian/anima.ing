mod record;
mod scan;

use std::{sync::Mutex, time::Duration};

use super::{PendingChild, Shared, locked};
use uuid::Uuid;

pub(in crate::threads::holder) fn reserve(shared: &Mutex<Shared>, runner: Uuid) -> bool {
    let dir = locked(shared).dir.clone();
    matches!(record::read(&dir), Ok(None)) && record::save(&dir, Some(runner))
}

pub(super) fn attached(shared: &Mutex<Shared>) {
    let state = locked(shared);
    if state.write_record_checked() {
        record::save(&state.dir, None);
    }
}

pub(super) async fn keep_or_end(shared: &Mutex<Shared>, mut pending: PendingChild) {
    if pending.due() && pending.end().await {
        record::save(&locked(shared).dir, None);
    } else {
        locked(shared).remote_child = Some(pending);
    }
}

pub(in crate::threads::holder) async fn reap_pending(
    shared: &Mutex<Shared>,
    turn: &tokio::sync::Mutex<()>,
) {
    let _only = turn.lock().await;
    let pending = locked(shared).remote_child.take();
    if let Some(pending) = pending {
        keep_or_end(shared, pending).await;
    } else {
        recover_stale(shared).await;
    }
}

pub(in crate::threads::holder) async fn end_pending(shared: &Mutex<Shared>) {
    let pending = locked(shared).remote_child.take();
    if let Some(mut pending) = pending
        && pending.end().await
    {
        record::save(&locked(shared).dir, None);
    }
}

pub(in crate::threads::holder) async fn recover_stale(shared: &Mutex<Shared>) {
    let dir = locked(shared).dir.clone();
    let Ok(Some(runner)) = record::read(&dir) else {
        return;
    };
    let attached_here = locked(shared).threads.contains_key(&runner.to_string());
    if attached_here {
        attached(shared);
    } else if scan::sweep(runner).await {
        record::save(&dir, None);
    }
}

impl PendingChild {
    fn due(&self) -> bool {
        self.ending
            || matches!(self.child.exited(), Ok(true))
            || self.since.elapsed() >= Duration::from_secs(30)
    }

    async fn end(&mut self) -> bool {
        self.ending = true;
        let reaped = self.child.end().await.inspect_err(|error| {
            tracing::warn!(%error, "could not reap a pending remote Code child");
        });
        let ended = scan::sweep(self.request.runner).await;
        if !ended {
            tracing::warn!("could not prove that every pending remote Code descendant ended");
        }
        reaped.is_ok() && ended
    }
}
