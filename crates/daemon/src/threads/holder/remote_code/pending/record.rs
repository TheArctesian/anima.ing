//! The holder records a pending launch before spawning it. Recovery searches for
//! its fresh thread mark and never restores a numeric process or group ID.

use std::path::Path;

use uuid::Uuid;

use crate::config::private_file;

const FILE: &str = "pending-remote-child.json";

pub(super) fn save(dir: &Path, runner: Option<Uuid>) -> bool {
    let bytes = serde_json::to_vec(&runner).expect("an optional UUID serializes");
    private_file::write(dir, FILE, &bytes)
        .inspect_err(|error| tracing::warn!(%error, "could not store pending remote cleanup"))
        .is_ok()
}

pub(super) fn read(dir: &Path) -> Result<Option<Uuid>, ()> {
    let Some(bytes) = private_file::read_within(&dir.join(FILE), 40).map_err(|_| ())? else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes).map_err(|_| ())
}
