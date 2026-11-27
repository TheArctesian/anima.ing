use std::collections::{BTreeMap, BTreeSet};

use daemon_core::wire::{
    Batch, PushAccepted, ThreadSummaries, ThreadSummariesReceipt, ThreadSummary,
};
use uuid::Uuid;

use crate::{
    config::{agent_usage::UsageSettings, home::Home},
    remote_code::{Store, launch::publication::matching_summary},
};

use super::Pusher;

type Summaries = (ThreadSummaries, UsageSettings, BTreeSet<Uuid>);

#[derive(Default)]
pub(super) struct Pending {
    count: Option<(u32, i64, UsageSettings)>,
    count_revision: i64,
    rows: BTreeMap<Uuid, ThreadSummary>,
    acknowledged: BTreeMap<Uuid, i64>,
    priority: BTreeSet<Uuid>,
}

impl Pending {
    pub(super) fn replace(&mut self, batch: Option<Summaries>) {
        let Some((batch, policy, priority)) = batch else {
            *self = Self::default();
            return;
        };
        self.acknowledged
            .retain(|id, _| batch.summaries.iter().any(|row| row.runner_id == *id));
        self.count = Some((batch.count, batch.count_revision, policy));
        self.priority = priority;
        self.rows = batch
            .summaries
            .into_iter()
            .filter(|row| {
                self.acknowledged
                    .get(&row.runner_id)
                    .is_none_or(|revision| *revision < row.revision)
            })
            .map(|row| (row.runner_id, row))
            .collect();
    }

    pub(super) fn outgoing(&self, home: Option<&Home>) -> Option<ThreadSummaries> {
        let (count, count_revision, policy) = self.count.as_ref()?;
        if UsageSettings::read(home?).ok().as_ref() != Some(policy) {
            return None;
        }
        let (count, count_revision) = (*count, *count_revision);
        (self.count_revision < count_revision || !self.rows.is_empty()).then(|| ThreadSummaries {
            count,
            count_revision,
            summaries: self
                .priority
                .iter()
                .filter_map(|id| self.rows.get(id))
                .chain(
                    self.rows
                        .values()
                        .filter(|row| !self.priority.contains(&row.runner_id)),
                )
                .take(256)
                .cloned()
                .collect(),
        })
    }

    fn acknowledge(&mut self, sent: &ThreadSummaries, receipt: &ThreadSummariesReceipt) {
        if receipt.count_revision >= sent.count_revision {
            self.count_revision = self.count_revision.max(receipt.count_revision);
        }
        for row in sent
            .summaries
            .iter()
            .filter_map(|row| matching_summary(row, receipt))
        {
            self.acknowledged
                .entry(row.runner_id)
                .and_modify(|old| *old = (*old).max(row.revision))
                .or_insert(row.revision);
            if self
                .rows
                .get(&row.runner_id)
                .is_some_and(|pending| pending.revision <= row.revision)
            {
                self.rows.remove(&row.runner_id);
            }
        }
    }
}

impl Pusher {
    pub(super) async fn accept_thread_summaries(&mut self, batch: &Batch, response: &PushAccepted) {
        let (Some(sent), Some(receipt)) = (&batch.agent_threads, &response.agent_threads) else {
            return;
        };
        if let Some(home) = &self.marks
            && sent
                .summaries
                .iter()
                .any(|row| self.thread_summaries.priority.contains(&row.runner_id))
        {
            let home = home.clone();
            let device = self.device;
            let hub = self.credential.hub.clone();
            let stamp = self.credential.token.hash().to_bytes();
            let sent = sent.clone();
            let receipt = receipt.clone();
            let stored = tokio::task::spawn_blocking(move || {
                let current = crate::remote_code::delivery::Delivery::enrolled(&home).ok();
                if !current.is_some_and(|delivery| {
                    delivery.config().device == device
                        && delivery.config().hub == hub
                        && delivery.credential_stamp() == stamp
                }) {
                    return false;
                }
                Store::at(&home)
                    .acknowledge_publication((device, &hub, stamp), &sent, &receipt)
                    .is_ok()
            })
            .await
            .unwrap_or(false);
            if !stored {
                return;
            }
        }
        self.thread_summaries.acknowledge(sent, receipt);
    }
}
