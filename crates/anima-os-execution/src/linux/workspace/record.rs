use super::{Error, Result};
use anima_os_maintenance::capacity::Resources;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const ROOT: &str = "/persist/anima-workspaces";

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Preparing,
    Prepared,
    Starting,
    Running,
    Stopping,
    Stopped,
    Unknown,
    Deleting,
    Deleted,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub schema: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<super::checkout::Receipt>,
    #[serde(default)]
    pub principal: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handoff: Option<super::handoff::Binding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) incoming: Option<super::handoff::restore::Incoming>,
    pub workspace: Uuid,
    pub storage_bytes: u64,
    pub generation: u64,
    pub run: Option<Uuid>,
    pub phase: Phase,
    pub resources: Option<Resources>,
    pub image: Option<String>,
}

impl Record {
    pub fn new(workspace: Uuid, storage_bytes: u64) -> Self {
        Self {
            schema: 1,
            checkout: None,
            principal: None,
            handoff: None,
            incoming: None,
            workspace,
            storage_bytes,
            generation: 0,
            run: None,
            phase: Phase::Preparing,
            resources: None,
            image: None,
        }
    }

    pub fn container(&self) -> String {
        format!("anima-workspace-{}-{}", self.workspace, self.generation)
    }
    pub fn network(&self) -> String {
        format!("anima-workspace-{}", self.workspace)
    }

    pub fn checkout_ready(&self) -> bool {
        self.principal.is_none()
            || self
                .checkout
                .as_ref()
                .is_none_or(super::checkout::Receipt::ready)
    }

    pub(super) fn require_launchable(&self) -> Result<()> {
        if self.handoff.is_some() {
            return Err(Error::Refused(
                "This workspace generation is fenced for a handoff.",
            ));
        }
        if self
            .incoming
            .as_ref()
            .is_some_and(|receipt| !receipt.activated())
        {
            return Err(Error::Refused(
                "The restored workspace awaits handoff activation.",
            ));
        }
        Ok(())
    }
}

pub fn home(id: Uuid) -> PathBuf {
    Path::new(ROOT).join("homes").join(id.to_string())
}
fn path(id: Uuid) -> PathBuf {
    Path::new(ROOT).join(format!("{id}.json"))
}

pub(super) fn locks() -> PathBuf {
    Path::new(ROOT).join("locks")
}

pub fn lock(id: Uuid) -> Result<File> {
    if id.is_nil() {
        return Err(Error::Refused("Choose a valid workspace identity."));
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(locks().join(id.to_string()))?;
    file.try_lock().map_err(|error| match error {
        fs::TryLockError::WouldBlock => Error::WorkspaceBusy,
        fs::TryLockError::Error(error) => Error::Io(error),
    })?;
    Ok(file)
}

pub fn read(id: Uuid) -> Result<Option<Record>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path(id))
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(16385).read_to_end(&mut bytes)?;
    if bytes.len() > 16384 {
        return Err(Error::Refused("Workspace metadata exceeded its limit."));
    }
    let record: Record = serde_json::from_slice(&bytes)?;
    if record.schema != 1 || record.workspace != id {
        return Err(Error::Refused("Workspace metadata needs reconciliation."));
    }
    if let Some(binding) = record.handoff {
        super::handoff::validate_record(binding, &record)?;
    }
    if let Some(incoming) = &record.incoming {
        incoming.validate(&record)?;
    }
    Ok(Some(record))
}

pub fn required(id: Uuid) -> Result<Record> {
    read(id)?.ok_or(Error::Refused("Prepare the workspace first."))
}

pub fn save(record: &Record) -> Result<()> {
    let pending = Path::new(ROOT).join(format!("{}.pending", record.workspace));
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&pending)?;
    file.write_all(&serde_json::to_vec(record)?)?;
    file.sync_all()?;
    fs::rename(pending, path(record.workspace))?;
    sync()
}

/// Remove an unpublished import after its transfer has been durably sealed.
pub(super) fn remove_import(id: Uuid) -> Result<()> {
    fs::remove_file(path(id))?;
    sync()
}

pub(super) fn sync() -> Result<()> {
    File::open(ROOT)?.sync_all()?;
    Ok(())
}

pub struct Allocation {
    _file: File,
}

pub fn allocation() -> Result<Allocation> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(Path::new(ROOT).join("allocation.lock"))?;
    file.try_lock()
        .map_err(|_| Error::Refused("Another workspace is reserving disk space."))?;
    Ok(Allocation { _file: file })
}

pub(in crate::linux) fn free_storage(_allocation: &Allocation) -> Result<u64> {
    let capacity = nix::sys::statvfs::statvfs("/persist").map_err(std::io::Error::from)?;
    let available = capacity
        .blocks_available()
        .checked_mul(capacity.fragment_size())
        .ok_or(Error::Refused("Node disk capacity overflowed."))?;
    Ok(available
        .saturating_sub(writable_allowances()?)
        .saturating_sub(32 * 1024_u64.pow(3)))
}

fn writable_allowances() -> Result<u64> {
    let mut bytes = 0_u64;
    for entry in fs::read_dir(ROOT)? {
        let entry = entry?;
        if entry
            .path()
            .extension()
            .is_none_or(|extension| extension != "json")
        {
            continue;
        }
        let id = entry
            .path()
            .file_stem()
            .and_then(|name| name.to_str())
            .and_then(|id| Uuid::parse_str(id).ok())
            .ok_or(Error::Refused("Workspace quota inventory is invalid."))?;
        bytes = bytes
            .checked_add(required(id)?.storage_bytes)
            .ok_or(Error::Refused("Workspace quota inventory overflowed."))?;
    }
    let captured = super::context::reserved_bytes()?;
    let builds = super::super::builds::reserved_bytes()?;
    bytes
        .checked_add(captured)
        .and_then(|bytes| bytes.checked_add(builds))
        .ok_or(Error::Refused("Disk reservation accounting overflowed."))
}

pub fn check_run_owner(workspace: Uuid, run: Uuid) -> Result<()> {
    for entry in fs::read_dir(ROOT)? {
        let path = entry?.path();
        if path.extension().is_none_or(|extension| extension != "json") {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|name| name.to_str())
            .and_then(|name| Uuid::parse_str(name).ok())
            .ok_or(Error::Refused("Workspace identity inventory is invalid."))?;
        if id != workspace && required(id)?.run == Some(run) {
            return Err(Error::Refused(
                "The run already belongs to another workspace.",
            ));
        }
    }
    Ok(())
}
