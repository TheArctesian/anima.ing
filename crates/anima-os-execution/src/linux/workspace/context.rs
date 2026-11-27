mod capture;
mod cleanup;
pub use capture::capture;
mod lease;
pub use lease::{pin, unpin};
#[cfg(test)]
pub(in crate::linux) mod tests;
mod tree;
use super::{Error, Result, command, record};
pub(super) use cleanup::require_released;
pub use cleanup::{release, status};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};
use uuid::Uuid;

const ROOT: &str = "/persist/anima-workspaces/contexts";
const QUOTA: u64 = 4 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub workspace: Uuid,
    pub generation: u64,
    pub context: Uuid,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Context {
    schema: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<super::checkout::Source>,
    id: Uuid,
    workspace: Uuid,
    generation: u64,
    phase: String,
    digest: Option<String>,
    bytes: u64,
    entries: usize,
    quota_bytes: u64,
}

fn path(path: &Path) -> Result<&str> {
    path.to_str()
        .ok_or(Error::Refused("Source context path is invalid."))
}

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)?;
    let mut bytes = Vec::new();
    file.take(16385).read_to_end(&mut bytes)?;
    if bytes.len() > 16384 {
        return Err(Error::Refused("Source receipt exceeded its limit."));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn save(path: &Path, context: &Context) -> Result<()> {
    let pending = path.with_extension("pending");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&pending)?;
    file.write_all(&serde_json::to_vec(context)?)?;
    file.sync_all()?;
    fs::rename(pending, path)?;
    fs::File::open(
        path.parent()
            .ok_or(Error::Refused("Invalid receipt path."))?,
    )?
    .sync_all()?;
    Ok(())
}

pub fn reserved_bytes() -> Result<u64> {
    if !Path::new(ROOT).exists() {
        return Ok(0);
    }
    let mut total = 0_u64;
    for entry in fs::read_dir(ROOT)? {
        let entry = entry?;
        let id = entry
            .file_name()
            .to_str()
            .and_then(|value| Uuid::parse_str(value).ok())
            .ok_or(Error::Refused("Source context inventory is invalid."))?;
        let context: Context = read(&entry.path().join("context.json"))?;
        if context.schema != 1
            || context.id != id
            || (context.quota_bytes != QUOTA
                && !(context.phase == "deleted" && context.quota_bytes == 0))
        {
            return Err(Error::Refused("Source context quota needs reconciliation."));
        }
        // A captured receipt is saved only after sealing the filtered source
        // and removing the staging snapshot. Its contents cannot grow.
        if context.phase != "captured" || context.digest.is_none() {
            total = total
                .checked_add(context.quota_bytes)
                .ok_or(Error::Refused("Source context quota overflowed."))?;
        }
    }
    Ok(total)
}

pub(in crate::linux) async fn release_if_present(
    workspace: Uuid,
    generation: u64,
    id: Uuid,
) -> Result<()> {
    if Path::new(ROOT).join(id.to_string()).try_exists()? {
        release(workspace, generation, id, true).await?;
    }
    Ok(())
}
