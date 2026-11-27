//! Bounded root-owned intent journal. Unresolved requests are never removed automatically.
use super::{Error, Receipt, Result};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use uuid::Uuid;
const ROOT: &str = "/persist/anima-workspace-requests";
const MAX_BYTES: u64 = 4096;

pub(super) struct Store {
    _lock: File,
}
impl Store {
    pub fn open() -> Result<Self> {
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(Path::new(ROOT).join("lifecycle.lock"))?;
        lock.try_lock()
            .map_err(|_| Error::Refused("Another workspace lifecycle operation is active."))?;
        Ok(Self { _lock: lock })
    }
    #[expect(
        clippy::unused_self,
        reason = "The borrowed store keeps the lifecycle journal lock held during reads."
    )]
    pub fn read(&self, request: Uuid) -> Result<Option<Receipt>> {
        let file = match OpenOptions::new()
            .read(true)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(path(request))
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if !file.metadata()?.is_file() {
            return Err(Error::Refused("Invalid workspace request receipt."));
        }
        let mut bytes = Vec::new();
        file.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(Error::Refused("Workspace receipt exceeds its bound."));
        }
        let receipt: Receipt = serde_json::from_slice(&bytes)?;
        if receipt.command.request != request || !receipt.command.fits() {
            return Err(Error::Refused(
                "Workspace receipt identity is inconsistent.",
            ));
        }
        Ok(Some(receipt))
    }
    pub fn reserve(&self, receipt: &Receipt) -> Result<()> {
        let limit = if matches!(
            receipt.command.action,
            agent_threads::managed::workspace::Action::Stop { .. }
                | agent_threads::managed::workspace::Action::Reconcile { .. }
        ) {
            4096
        } else {
            4032
        };
        let mut count = 0;
        for entry in fs::read_dir(ROOT)? {
            if entry?
                .path()
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                count += 1;
            }
            if count >= limit {
                return Err(Error::Refused(
                    "Workspace request history is full; retain unresolved receipts during archival.",
                ));
            }
        }
        self.save(receipt)
    }
    #[expect(
        clippy::unused_self,
        reason = "The borrowed store keeps the lifecycle journal lock held during writes."
    )]
    pub fn save(&self, receipt: &Receipt) -> Result<()> {
        let bytes = serde_json::to_vec(receipt)?;
        if bytes.len() as u64 > MAX_BYTES {
            return Err(Error::Refused("Workspace receipt exceeds its bound."));
        }
        let pending = Path::new(ROOT).join("receipt.pending");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(&pending)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(pending, path(receipt.command.request))?;
        File::open(ROOT)?.sync_all()?;
        Ok(())
    }
}
fn path(request: Uuid) -> PathBuf {
    Path::new(ROOT).join(format!("{request}.json"))
}
