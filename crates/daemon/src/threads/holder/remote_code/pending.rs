use std::{sync::Mutex, time::Duration};

use super::{PendingChild, Shared, locked};

pub(super) async fn keep_or_end(shared: &Mutex<Shared>, mut pending: PendingChild) {
    if !pending.due() || !pending.end().await {
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
    }
}

pub(in crate::threads::holder) async fn end_pending(shared: &Mutex<Shared>) {
    let pending = locked(shared).remote_child.take();
    if let Some(mut pending) = pending {
        let _ = pending.end().await;
    }
}

impl PendingChild {
    fn due(&self) -> bool {
        matches!(self.child.exited(), Ok(true)) || self.since.elapsed() >= Duration::from_secs(30)
    }

    async fn end(&mut self) -> bool {
        self.child
            .end()
            .await
            .inspect_err(|error| {
                tracing::warn!(%error, "could not reap a pending remote Code child");
            })
            .is_ok()
            && self.sweep_marks().await
    }

    async fn sweep_marks(&self) -> bool {
        let thread = self.request.runner.to_string();
        let ended =
            crate::threads::marker::end_marked(&thread, &crate::threads::marker::Keep::nothing())
                .await;
        if !ended.all {
            tracing::warn!("could not prove that every pending remote Code descendant ended");
        }
        ended.all
    }
}
