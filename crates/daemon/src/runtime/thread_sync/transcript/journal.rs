use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    Home, MAX_PENDING, ThreadEntry, ThreadUploadReceipt, Update, UsageSettings, authorized,
    content, earlier,
};

#[derive(Default, Deserialize, Serialize)]
pub(super) struct Journal {
    pub(super) observed: u64,
    pub(super) accepted: i64,
    pub(super) pending: Vec<ThreadEntry>,
    #[serde(default)]
    pub(super) confirmed: Option<ThreadUploadReceipt>,
}

impl Journal {
    pub(super) async fn refresh(
        &mut self,
        home: &Home,
        runner: Uuid,
        policy: &UsageSettings,
    ) -> Option<()> {
        if self.pending.len() < MAX_PENDING {
            let updates = earlier(runner, self.observed).await;
            authorized(home, policy).then_some(())?;
            self.record(updates);
        }
        authorized(home, policy).then_some(())
    }

    pub(super) fn note_receipt(
        &mut self,
        receipt: ThreadUploadReceipt,
        through: i64,
    ) -> Option<()> {
        if receipt.id.is_nil() {
            return None;
        }
        self.accepted = self.accepted.max(receipt.last_sequence);
        self.pending.retain(|entry| entry.sequence > self.accepted);
        if receipt.last_sequence >= through {
            self.confirmed = Some(receipt);
        }
        Some(())
    }
}
impl Journal {
    pub(super) fn record(&mut self, updates: Vec<Update>) {
        for update in updates {
            if self.pending.len() == MAX_PENDING {
                break;
            }
            self.observed = self.observed.max(update.seq);
            let Some(content) = content(update.change) else {
                continue;
            };
            let sequence = self.accepted + i64::try_from(self.pending.len()).unwrap_or(0) + 1;
            self.pending.push(ThreadEntry { sequence, content });
        }
    }
}
