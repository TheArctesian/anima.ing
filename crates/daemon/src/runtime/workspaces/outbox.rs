//! Durable delivery intent and receipts; never redispatch an ambiguous local operation.
use crate::config::{home::Home, private_file};
use agent_threads::managed::workspace::{Command, Receipt};
use serde::{Deserialize, Serialize};
use std::{fs, os::unix::fs::DirBuilderExt, path::PathBuf};
use uuid::Uuid;
const LIMIT: usize = 64;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Entry {
    pub origin: String,
    pub device: Uuid,
    pub receipt: Receipt,
    pub finished: bool,
}
pub(super) struct Outbox {
    dir: PathBuf,
}
impl Outbox {
    pub fn open(home: &Home) -> Option<Self> {
        let dir = home.dir().join("workspace-outbox");
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
            if ids.len() > LIMIT {
                return None;
            }
        }
        ids.sort_unstable();
        Some(ids)
    }
    pub fn room(&self) -> bool {
        self.ids().is_some_and(|ids| ids.len() < LIMIT)
    }
    pub fn read(&self, id: Uuid) -> Option<Entry> {
        let bytes = private_file::read_within(&self.path(id), 8192).ok()??;
        let entry: Entry = serde_json::from_slice(&bytes).ok()?;
        (entry.receipt.command.request == id && entry.receipt.fits()).then_some(entry)
    }
    pub fn reserve(&self, command: Command, origin: String, device: Uuid) -> Option<Entry> {
        // Existing intent means delivery may already have happened, even without a receipt.
        if self.path(command.request).exists() || !self.room() {
            return None;
        }
        let entry = Entry {
            origin,
            device,
            finished: false,
            receipt: Receipt {
                command,
                result: None,
                failure: None,
            },
        };
        self.save(&entry).then_some(entry)
    }
    pub fn save(&self, entry: &Entry) -> bool {
        let Ok(bytes) = serde_json::to_vec(entry) else {
            return false;
        };
        bytes.len() <= 8192
            && entry.receipt.fits()
            && private_file::write(
                &self.dir,
                &format!("{}.json", entry.receipt.command.request),
                &bytes,
            )
            .is_ok()
    }
    pub fn acknowledge(&self, entry: &Entry) -> bool {
        !entry.finished
            || (fs::remove_file(self.path(entry.receipt.command.request)).is_ok()
                && fs::File::open(&self.dir)
                    .and_then(|dir| dir.sync_all())
                    .is_ok())
    }
    fn path(&self, id: Uuid) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }
}
