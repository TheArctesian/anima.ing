//! Retain the console request across authenticated sessions in the same boot.
use crate::linux::{Error, Result, command};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read as _},
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _},
    path::Path,
};
use uuid::Uuid;

const LOCK: &str = "/run/anima-console-diagnostic.lock";
const PATH: &str = "/run/anima-console-diagnostic.json";
const MAX_BYTES: u64 = 1024;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    request: Uuid,
    operation: Option<Uuid>,
    revision: u64,
}

pub(super) struct Pending {
    record: Record,
    lock: File,
}

impl Pending {
    pub(super) fn open() -> Result<(Self, &'static str)> {
        let lock = command::lock(LOCK)?;
        let (record, action) = if let Some(record) = load()? {
            (record, "job-get")
        } else {
            let record = Record {
                request: Uuid::now_v7(),
                operation: None,
                revision: 0,
            };
            save(&record)?;
            (record, "job-start")
        };
        Ok((Self { record, lock }, action))
    }

    pub(super) fn request(&self) -> Uuid {
        self.record.request
    }

    pub(super) fn has_observation(&self) -> bool {
        self.record.operation.is_some()
    }

    pub(super) fn observe<'a>(&mut self, value: &'a Value) -> Result<&'a str> {
        let operation = value["id"]
            .as_str()
            .and_then(|id| id.parse::<Uuid>().ok())
            .filter(|id| !id.is_nil())
            .ok_or(Error::Refused("Invalid diagnostic operation identity."))?;
        let revision = value["revision"]
            .as_u64()
            .ok_or(Error::Refused("Invalid diagnostic operation revision."))?;
        if self.record.operation.is_some_and(|id| id != operation)
            || revision < self.record.revision
        {
            return Err(Error::Refused(
                "The diagnostic operation changed unexpectedly.",
            ));
        }
        let state = value["state"]
            .as_str()
            .filter(|state| {
                matches!(
                    *state,
                    "queued" | "running" | "succeeded" | "cancelled" | "interrupted"
                )
            })
            .ok_or(Error::Refused("Invalid diagnostic operation state."))?;
        if self.record.operation.is_none() || self.record.revision != revision {
            self.record.operation = Some(operation);
            self.record.revision = revision;
            save(&self.record)?;
        }
        Ok(state)
    }

    pub(super) fn finish(self) -> Result<()> {
        fs::remove_file(PATH)?;
        drop(self.lock);
        Ok(())
    }
}

fn load() -> Result<Option<Record>> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(PATH)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let meta = file.metadata()?;
    if !meta.is_file() || meta.uid() != 0 || meta.mode() & 0o077 != 0 || meta.len() > MAX_BYTES {
        return Err(Error::Refused(
            "The saved diagnostic request is not protected.",
        ));
    }
    let mut contents = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut contents)?;
    let record: Record = serde_json::from_slice(&contents)?;
    if contents.len() as u64 > MAX_BYTES
        || record.request.is_nil()
        || record.operation.is_some_and(|id| id.is_nil())
    {
        return Err(Error::Refused("The saved diagnostic request is invalid."));
    }
    Ok(Some(record))
}

fn save(record: &Record) -> Result<()> {
    command::atomic(Path::new(PATH), &serde_json::to_vec(record)?, 0o600)
}
