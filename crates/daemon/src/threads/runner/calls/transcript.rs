//! Bounded forward projection; raw account data and queued prompt bodies stay local.
use super::super::Runner;
use crate::runtime::thread_content::content;
use agent_threads::{
    control::{Outcome, Refusal},
    managed::{Snapshot, ThreadSnapshot, TranscriptUpdate},
};
use uuid::Uuid;

impl Runner {
    pub(super) fn managed_transcript(&self, id: Uuid, after: u64) -> Outcome {
        let Some(thread) = self.threads.get(&id) else {
            return Outcome::Refused {
                why: Refusal::UnknownThread,
            };
        };
        if thread.row.open.len() > 1024 {
            return Outcome::Refused {
                why: Refusal::UnknownThread,
            };
        }
        let row = ThreadSnapshot {
            id,
            program: thread.row.program,
            state: thread.row.state,
            title: thread.row.title.chars().take(80).collect(),
            started: thread.row.started,
            open: thread.row.open.clone(),
        };
        let mut updates = Vec::new();
        let mut bytes = 0_usize;
        for update in thread.kept().filter(|update| update.seq > after).take(64) {
            let projected = TranscriptUpdate {
                seq: update.seq,
                content: content(update.change.clone()),
            };
            let size = serde_json::to_vec(&projected).map_or(usize::MAX, |value| value.len());
            if bytes.saturating_add(size) > 256 * 1024 {
                break;
            }
            bytes += size;
            updates.push(projected);
        }
        let mut current = Vec::new();
        for change in thread.now_changes().into_iter().take(64) {
            let Some(value) = content(change) else {
                continue;
            };
            let size = serde_json::to_vec(&value).map_or(usize::MAX, |value| value.len());
            if bytes.saturating_add(size) > 512 * 1024 {
                break;
            }
            bytes += size;
            current.push(value);
        }
        Outcome::ManagedTranscript {
            snapshot: Box::new(Snapshot {
                row,
                updates,
                current,
            }),
        }
    }
}
