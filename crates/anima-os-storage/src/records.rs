//! Read the installed volume list without following links or unbounded input.
use std::{
    collections::HashSet,
    fs::OpenOptions,
    io::{self, Read as _},
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    path::Path,
};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, Result};

pub const RECORD_NAME: &str = "data-volumes.json";
pub const MAX_VOLUMES: usize = 16;
const MAX_RECORD_BYTES: u64 = 16_384;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Volume {
    pub uuid: Uuid,
    pub model: Option<String>,
    pub serial: Option<String>,
}

pub(crate) fn read(path: &Path) -> Result<Option<Vec<Volume>>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(
            (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK)
                .bits()
                .cast_signed(),
        )
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return Err(Error::Refused(
            "The recorded volume list is not a protected root-owned file.",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_RECORD_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RECORD_BYTES {
        return Err(Error::Refused("The recorded volume list is too large."));
    }
    let volumes: Vec<Volume> = serde_json::from_slice(&bytes)?;
    let mut seen = HashSet::new();
    if volumes.len() > MAX_VOLUMES
        || volumes
            .iter()
            .any(|volume| volume.uuid.is_nil() || !seen.insert(volume.uuid))
    {
        return Err(Error::Refused(
            "The recorded volume identities are invalid or repeated.",
        ));
    }
    Ok(Some(volumes))
}
