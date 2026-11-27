//! Worker progress is an observation, never authority to release or deploy an artifact.
use super::super::store;
use crate::linux::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, os::unix::fs::OpenOptionsExt, path::Path};

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stage {
    WaitingForBuilder,
    Detecting,
    Building,
    CheckingExport,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Observation {
    schema: u32,
    stage: Stage,
    failed: bool,
}

pub fn observe(work: &Path) -> Result<Option<Observation>> {
    let path = work.join("progress.json");
    let value: Observation = match store::read(&path) {
        Ok(value) => value,
        Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if value.schema != 1 {
        return Err(Error::Refused("The build progress schema is incompatible."));
    }
    Ok(Some(value))
}

pub fn write(work: &Path, stage: Stage, failed: bool) -> Result<()> {
    let pending = work.join("progress.pending");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&pending)?;
    file.write_all(&serde_json::to_vec(&Observation {
        schema: 1,
        stage,
        failed,
    })?)?;
    file.sync_all()?;
    fs::rename(pending, work.join("progress.json"))?;
    fs::File::open(work)?.sync_all()?;
    Ok(())
}

pub fn failed(work: &Path) -> Result<()> {
    if let Some(previous) = observe(work)? {
        write(work, previous.stage, true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
