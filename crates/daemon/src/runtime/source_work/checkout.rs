//! Deliver a sole checkout request while the daemon continues reporting the job's lease.
use chrono::Utc;
use tokio::task::JoinSet;

use crate::source_work::Error;

use super::{Coordinator, HubTarget, Observation};

#[derive(Default)]
pub(super) struct Pending {
    tasks: JoinSet<()>,
}

impl Pending {
    pub(super) fn update(
        &mut self,
        owner: &mut Coordinator,
        target: &HubTarget,
        observation: Observation,
    ) -> Result<(), Error> {
        let _ = self.tasks.try_join_next();
        if self.tasks.is_empty()
            && let Some(checkout) =
                owner.checkout(observation.execution, &observation.status, Utc::now())?
        {
            let hub = target.hub.clone();
            self.tasks.spawn(async move {
                let credential = if checkout.fetch {
                    hub.checkout_token(&observation.token, checkout.held).await
                } else {
                    None
                };
                checkout.deliver(credential).await;
            });
        }
        Ok(())
    }
}
