//! Root-owned receipts keep payload digests, never prompts or provider credentials.
use super::super::{Error, Result};
use agent_threads::control::Outcome;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const ROOT: &str = "/persist/anima-commands";
const MAX_BYTES: u64 = 1_100_000;
const MAX_RECEIPTS: usize = 4096;
const MAX_STORAGE: u64 = 64 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Receipt {
    pub schema: u32,
    pub request: Uuid,
    pub workspace: Uuid,
    pub generation: u64,
    pub principal: Uuid,
    pub fingerprint: String,
    pub result: Option<Outcome>,
    #[serde(default)]
    pub thread: Option<Uuid>,
}

pub(super) struct Store {
    dir: PathBuf,
    workspace: Uuid,
    _lock: File,
}

impl Store {
    pub fn open(workspace: Uuid) -> Result<Self> {
        if workspace.is_nil() {
            return Err(Error::Refused("Choose a workspace identity."));
        }
        let dir = Path::new(ROOT).join(workspace.to_string());
        fs::create_dir_all(&dir)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(dir.join("command.lock"))?;
        lock.try_lock()
            .map_err(|_| Error::Refused("A command is still being delivered to this workspace."))?;
        Ok(Self {
            dir,
            workspace,
            _lock: lock,
        })
    }

    pub fn read(&self, request: Uuid) -> Result<Option<Receipt>> {
        let file = match OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(self.dir.join(format!("{request}.json")))
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(Error::Refused("The command receipt exceeded its limit."));
        }
        let receipt: Receipt = serde_json::from_slice(&bytes)?;
        if receipt.schema != 1 || receipt.request != request || receipt.workspace != self.workspace
        {
            return Err(Error::Refused("The command receipt needs reconciliation."));
        }
        Ok(Some(receipt))
    }

    pub fn reserve(&self, receipt: &Receipt) -> Result<()> {
        let mut count = 0;
        let mut bytes = 0_u64;
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|ext| ext == "json") {
                count += 1;
                bytes = bytes.saturating_add(entry.metadata()?.len());
            }
            if count >= MAX_RECEIPTS || bytes.saturating_add(MAX_BYTES) > MAX_STORAGE {
                return Err(Error::Refused(
                    "Command receipt storage is full. Retain unresolved receipts during archival.",
                ));
            }
        }
        self.save(receipt)
    }

    pub fn save(&self, receipt: &Receipt) -> Result<()> {
        let bytes = serde_json::to_vec(receipt)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(Error::Refused("The command receipt exceeded its limit."));
        }
        let pending = self.dir.join("receipt.pending");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(&pending)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(pending, self.dir.join(format!("{}.json", receipt.request)))?;
        File::open(&self.dir)?.sync_all()?;
        File::open(ROOT)?.sync_all()?;
        Ok(())
    }
}
