//! Retry receipts without retaining prompts. The original hub/device binding is immutable.
use crate::config::{home::Home, private_file};
use agent_threads::managed::{Command, Report};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    os::unix::fs::DirBuilderExt,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const LIMIT: usize = 64;
const BYTES: usize = 1_100_000;
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Entry {
    pub origin: String,
    pub device: Uuid,
    pub report: Report,
    pub finished: bool,
}
pub(super) struct Outbox {
    dir: PathBuf,
}
impl Outbox {
    pub fn open(home: &Home) -> Option<Self> {
        let dir = home.dir().join("managed-code-outbox");
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
        let bytes = private_file::read_within(&self.path(id), BYTES).ok()??;
        let entry: Entry = serde_json::from_slice(&bytes).ok()?;
        (entry.report.request == id).then_some(entry)
    }
    pub fn reserve(&self, command: &Command, origin: &str, device: Uuid) -> Option<Entry> {
        if self.path(command.request).exists() {
            let held = self.read(command.request)?;
            return (held.origin == origin
                && held.device == device
                && held.report.workspace == command.workspace
                && held.report.generation == command.generation
                && held.report.principal == command.principal)
                .then_some(held);
        }
        if !self.room() {
            return None;
        }
        let entry = Entry {
            origin: origin.to_owned(),
            device,
            finished: false,
            report: Report {
                request: command.request,
                workspace: command.workspace,
                generation: command.generation,
                principal: command.principal,
                result: None,
            },
        };
        self.save(&entry).then_some(entry)
    }
    pub fn save(&self, entry: &Entry) -> bool {
        let Ok(bytes) = serde_json::to_vec(entry) else {
            return false;
        };
        bytes.len() <= BYTES
            && private_file::write(&self.dir, &format!("{}.json", entry.report.request), &bytes)
                .is_ok()
    }
    pub fn acknowledge(&self, entry: &Entry) -> bool {
        if !entry.finished {
            return true;
        }
        if !super::super::managed_transcripts::register(
            &self.dir,
            &entry.origin,
            entry.device,
            &entry.report,
        ) {
            return false;
        }
        fs::remove_file(self.path(entry.report.request)).is_ok()
            && fs::File::open(&self.dir)
                .and_then(|dir| dir.sync_all())
                .is_ok()
    }
    fn path(&self, id: Uuid) -> PathBuf {
        Path::new(&self.dir).join(format!("{id}.json"))
    }
}
