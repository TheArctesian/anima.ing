//! Run host attempts separately from the source owner while sharing the hub claim.
mod apply;
mod execution;
mod journal;
mod reports;
#[cfg(test)]
mod tests;

use std::{
    io,
    sync::{Arc, Mutex},
};

use chrono::Utc;
use daemon_core::wire::{DaemonToken, Held, Lease, WorkState, WorkType};
use serde_json::Value;
use tokio::task::JoinSet;

use crate::{
    applications::{Client, identity},
    config::home::Home,
    hub::answer::Answer,
};

use self::{
    execution::{Authority, relinquished},
    journal::Journal,
};
use super::HubTarget;

type State = Arc<Mutex<Journal>>;

pub(super) struct Hosting {
    client: Arc<Client>,
    journal: State,
    commands: JoinSet<()>,
    delivery: JoinSet<()>,
}

impl Hosting {
    pub(super) fn open(home: &Home, target: &HubTarget) -> Option<Self> {
        let client = Client::enrolled(&target.origin, target.device)?;
        let journal = Journal::open(home, target.device, target.origin.as_text())
            .map_err(|_| {
                tracing::warn!("Hosting work journal could not be opened.");
            })
            .ok()?;
        Some(Self {
            client: Arc::new(client),
            journal: Arc::new(Mutex::new(journal)),
            commands: JoinSet::new(),
            delivery: JoinSet::new(),
        })
    }

    pub(super) fn holds(&self) -> Vec<Held> {
        self.journal
            .lock()
            .map_or_else(|_| Vec::new(), |journal| journal.holds())
    }

    pub(super) async fn available(&self) -> Vec<WorkType> {
        let room = self
            .journal
            .lock()
            .is_ok_and(|journal| journal.has_room(Utc::now()));
        if !room {
            return Vec::new();
        }
        let (actions, apply) = tokio::join!(self.client.paired(), self.client.supports_apply());
        [
            actions.then_some(WorkType::ContainerAction),
            apply.then_some(WorkType::ApplicationApply),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    pub(super) fn submit(&mut self, target: &HubTarget, token: Arc<DaemonToken>, item: Value) {
        let Ok(held) = identity(&item) else { return };
        let Ok(lease) = serde_json::from_value::<Lease>(item["lease"].clone()) else {
            return;
        };
        if lease.until <= Utc::now()
            || !write(&self.journal, |journal| journal.reserve(held, Utc::now()))
        {
            return;
        }
        let authority = Authority {
            hub: target.hub.clone(),
            token,
        };
        self.commands.spawn(execution::run(
            self.journal.clone(),
            self.client.clone(),
            authority,
            (held, item),
        ));
    }

    pub(super) fn tick(&mut self, target: &HubTarget, token: &Arc<DaemonToken>) {
        while let Some(result) = self.commands.try_join_next() {
            if result.is_err() {
                tracing::warn!("A host action ended without a report; it will not be restarted.");
            }
        }
        let _ = self.delivery.try_join_next();
        if self.delivery.is_empty()
            && self
                .journal
                .lock()
                .is_ok_and(|journal| journal.pending().next().is_some())
        {
            let authority = Authority {
                hub: target.hub.clone(),
                token: token.clone(),
            };
            self.delivery.spawn(drain(self.journal.clone(), authority));
        }
    }
}

type Pending = (Held, daemon_core::wire::WorkReport, bool);

async fn drain(state: State, authority: Authority) {
    let pending: Vec<Pending> = match state.lock() {
        Ok(journal) => journal.pending().collect(),
        Err(_) => return,
    };
    for (held, report, finishing) in pending {
        deliver(&state, &authority, (held, report, finishing)).await;
    }
}

async fn deliver(state: &State, authority: &Authority, (held, mut report, finishing): Pending) {
    if report.state == WorkState::Done && !finishing {
        let phase = reports::phase(held, WorkState::Finishing);
        match reports::send(&authority.hub, &authority.token, &phase).await {
            Answer::Accepted(reply) => {
                if let Some(stop) = reply.stop {
                    let seconds = report.result.as_ref().map_or(0, |result| result.seconds);
                    report = reports::stopped(held, stop.reason, seconds);
                }
                if !write(state, |journal| journal.finished(held, report.clone())) {
                    return;
                }
            }
            Answer::Refused(code) if relinquished(code) => {
                write(state, |journal| journal.settle(held, Utc::now()));
                return;
            }
            _ => return,
        }
    }
    match reports::send(&authority.hub, &authority.token, &report).await {
        Answer::Accepted(_) => {
            write(state, |journal| journal.settle(held, Utc::now()));
        }
        Answer::Refused(code) if relinquished(code) => {
            write(state, |journal| journal.settle(held, Utc::now()));
        }
        _ => (),
    }
}

fn write(state: &State, change: impl FnOnce(&mut Journal) -> io::Result<()>) -> bool {
    let saved = state
        .lock()
        .is_ok_and(|mut journal| change(&mut journal).is_ok());
    if !saved {
        tracing::warn!("Hosting work journal did not confirm a transition.");
    }
    saved
}
