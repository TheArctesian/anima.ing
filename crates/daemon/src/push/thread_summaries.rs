use std::collections::BTreeMap;

use daemon_core::wire::{ThreadSummaries, ThreadSummariesReceipt, ThreadSummary};
use uuid::Uuid;

use crate::config::{agent_usage::UsageSettings, home::Home};

#[derive(Default)]
pub(super) struct Pending {
    count: Option<(u32, i64, UsageSettings)>,
    count_revision: i64,
    rows: BTreeMap<Uuid, ThreadSummary>,
    acknowledged: BTreeMap<Uuid, i64>,
}

impl Pending {
    pub(super) fn replace(&mut self, batch: Option<(ThreadSummaries, UsageSettings)>) {
        let Some((batch, policy)) = batch else {
            *self = Self::default();
            return;
        };
        self.acknowledged
            .retain(|id, _| batch.summaries.iter().any(|row| row.runner_id == *id));
        self.count = Some((batch.count, batch.count_revision, policy));
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
            summaries: self.rows.values().take(256).cloned().collect(),
        })
    }

    pub(super) fn acknowledge(&mut self, receipt: Option<&ThreadSummariesReceipt>) {
        let Some(receipt) = receipt else { return };
        self.count_revision = self.count_revision.max(receipt.count_revision);
        for row in &receipt.summaries {
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
