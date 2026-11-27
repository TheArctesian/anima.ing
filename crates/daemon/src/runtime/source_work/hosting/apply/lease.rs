//! Confirm each apply phase and bound local work by the last acknowledged lease.
use std::{future::Future, pin::pin, time::Duration};

use chrono::Utc;
use daemon_core::wire::{Held, RefusalCode, Reported, WorkState};
use tokio::time::{self, Instant};

use super::{End, reports};
use crate::{
    hub::answer::Answer,
    runtime::source_work::hosting::execution::{Authority, relinquished},
};

const HEARTBEAT: Duration = Duration::from_secs(10);
const RETRY: Duration = Duration::from_millis(250);

pub(super) struct Lease<'a> {
    authority: &'a Authority,
    item: Held,
    until: Instant,
    deadline: Instant,
    report_at: Instant,
}

impl<'a> Lease<'a> {
    pub(super) fn new(
        (authority, item): (&'a Authority, Held),
        initial: daemon_core::wire::Lease,
        lifetime: Duration,
    ) -> Self {
        let now = Instant::now();
        Self {
            authority,
            item,
            until: projected(initial),
            deadline: now + lifetime,
            report_at: now,
        }
    }

    pub(super) fn end(&self) -> Instant {
        self.until.min(self.deadline)
    }

    pub(super) async fn confirm(&mut self, phase: WorkState) -> Result<Reported, End> {
        let report = reports::phase(self.item, phase);
        let started = Instant::now();
        let ack = time::timeout_at(self.end(), async {
            loop {
                match reports::send(&self.authority.hub, &self.authority.token, &report).await {
                    Answer::Accepted(ack) => return Ok(ack),
                    Answer::Refused(code) if relinquished(code) => return Err(End::Lost),
                    Answer::Refused(RefusalCode::Unavailable) => time::sleep(RETRY).await,
                    _ => return Err(End::Failed("orphan")),
                }
            }
        })
        .await
        .map_err(|_| End::Expired)??;
        if let Some(order) = ack.stop {
            return Err(End::Stopped(order.reason));
        }
        self.until = projected(ack.lease);
        self.report_at = started + HEARTBEAT;
        if self.end() <= Instant::now() {
            return Err(End::Expired);
        }
        Ok(ack)
    }

    pub(super) async fn while_held<T>(
        &mut self,
        phase: WorkState,
        future: impl Future<Output = T>,
    ) -> Result<T, End> {
        let mut future = pin!(future);
        loop {
            tokio::select! {
                biased;
                () = time::sleep_until(self.end()) => return Err(End::Expired),
                value = &mut future => return Ok(value),
                () = time::sleep_until(self.report_at) => { self.confirm(phase).await?; }
            }
        }
    }
}

fn projected(lease: daemon_core::wire::Lease) -> Instant {
    let left = (lease.until - Utc::now()).to_std().unwrap_or_default();
    Instant::now() + left.min(Duration::from_secs(90))
}
