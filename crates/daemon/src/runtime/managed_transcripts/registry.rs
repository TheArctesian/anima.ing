use crate::config::private_file;
use agent_threads::managed::TranscriptUpdate;
use daemon_core::wire::{MAX_THREAD_ENTRY, ThreadEntry, ThreadSummary};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const MAX_THREADS: usize = 128;
const MAX_PENDING: usize = 128;
const LIMIT: usize = MAX_PENDING * (MAX_THREAD_ENTRY + 128) + 16_384;
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Journal {
    pub origin: String,
    pub device: Uuid,
    pub workspace: Uuid,
    pub generation: u64,
    pub thread: Uuid,
    pub observed: u64,
    #[serde(default)]
    pub terminal: bool,
    #[serde(default)]
    pub open: Vec<Uuid>,
    #[serde(default)]
    pub current: Vec<serde_json::Value>,
    pub accepted: i64,
    pub revision: i64,
    pub summary: Option<ThreadSummary>,
    pub pending: Vec<ThreadEntry>,
}
pub(super) struct Registry {
    dir: PathBuf,
}
impl Registry {
    pub fn open(home: &Path) -> Option<Self> {
        let dir = home.join("managed-code-transcripts");
        match fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => (),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(_) => return None,
        }
        private_file::folder_private(&dir)
            .ok()?
            .then_some(Self { dir })
    }
    pub fn ids(&self) -> Option<Vec<Uuid>> {
        let mut ids = Vec::new();
        for entry in fs::read_dir(&self.dir).ok()? {
            let path = entry.ok()?.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            ids.push(path.file_stem()?.to_str()?.parse().ok()?);
            if ids.len() > MAX_THREADS {
                return None;
            }
        }
        ids.sort_unstable();
        Some(ids)
    }
    pub fn read(&self, thread: Uuid) -> Option<Journal> {
        let bytes =
            private_file::read_within(&self.dir.join(format!("{thread}.json")), LIMIT).ok()??;
        let journal: Journal = serde_json::from_slice(&bytes).ok()?;
        (journal.thread == thread && journal.pending.len() <= MAX_PENDING).then_some(journal)
    }
    pub fn save(&self, journal: &Journal) -> bool {
        let Ok(bytes) = serde_json::to_vec(journal) else {
            return false;
        };
        bytes.len() <= LIMIT
            && private_file::write(&self.dir, &format!("{}.json", journal.thread), &bytes).is_ok()
    }
    pub fn archive(&self, journal: &Journal) -> bool {
        if !journal.terminal || !journal.pending.is_empty() {
            return false;
        }
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return false;
        };
        let count = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "closed"))
            .count();
        let closed = self.dir.join(format!("{}.closed", journal.thread));
        if count >= 4096 && !closed.exists() {
            return false;
        }
        let Ok(bytes) = serde_json::to_vec(journal) else {
            return false;
        };
        if bytes.len() > LIMIT
            || private_file::write(&self.dir, &format!("{}.closed", journal.thread), &bytes)
                .is_err()
        {
            return false;
        }
        fs::remove_file(self.dir.join(format!("{}.json", journal.thread))).is_ok()
            && fs::File::open(&self.dir)
                .and_then(|dir| dir.sync_all())
                .is_ok()
    }
    pub fn register(&self, journal: &Journal) -> bool {
        match private_file::read_within(&self.dir.join(format!("{}.closed", journal.thread)), LIMIT)
        {
            Ok(Some(bytes)) => {
                return serde_json::from_slice::<Journal>(&bytes).is_ok_and(|old| {
                    old.terminal
                        && old.pending.is_empty()
                        && old.thread == journal.thread
                        && old.origin == journal.origin
                        && old.device == journal.device
                        && old.workspace == journal.workspace
                        && old.generation == journal.generation
                });
            }
            Ok(None) => (),
            Err(_) => return false,
        }
        let Some(ids) = self.ids() else {
            return false;
        };
        if ids.contains(&journal.thread) {
            return self.read(journal.thread).is_some_and(|old| {
                old.origin == journal.origin
                    && old.device == journal.device
                    && old.workspace == journal.workspace
                    && old.generation == journal.generation
            });
        }
        ids.len() < MAX_THREADS && self.save(journal)
    }
}
impl Journal {
    pub fn record(&mut self, updates: Vec<TranscriptUpdate>) {
        for update in updates {
            if self.pending.len() >= MAX_PENDING || update.seq <= self.observed {
                continue;
            }
            if update.seq > self.observed.saturating_add(1) {
                self.append(json!({"change":"omitted","reason":"runner_history_gap"}));
                if self.pending.len() >= MAX_PENDING {
                    break;
                }
            }
            self.observed = update.seq;
            if let Some(content) = update.content {
                self.append(content);
            }
        }
    }
    pub fn append(&mut self, content: serde_json::Value) {
        if self.pending.len() >= MAX_PENDING {
            return;
        }
        let sequence = self
            .accepted
            .saturating_add(i64::try_from(self.pending.len()).unwrap_or(i64::MAX))
            .saturating_add(1);
        self.pending.push(ThreadEntry { sequence, content });
    }
}
