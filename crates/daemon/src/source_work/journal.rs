use super::Error;
use crate::config::{home::Home, private_file};
use daemon_core::{
    artifact::Receipt,
    wire::{WorkJob, WorkReport},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    os::unix::fs::{DirBuilderExt, MetadataExt},
    path::PathBuf,
};
use uuid::Uuid;
mod validation;
use validation::{validate, validate_job};

const LIMIT: usize = 128;
const BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Disposition {
    #[default]
    Attached,
    Lost,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Entry {
    pub execution: Uuid,
    pub device: Uuid,
    pub origin: String,
    pub job: WorkJob,
    pub report: Option<WorkReport>,
    pub settled: bool,
    #[serde(default)]
    pub start_requested: bool,
    #[serde(default)]
    pub stop_requested: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout_requested_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub disposition: Disposition,
    #[serde(default)]
    pub renewal: Option<daemon_core::source_jobs::Grant>,
    #[serde(default)]
    pub reported: Option<daemon_core::wire::WorkState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_receipt: Option<Receipt>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema: u32,
    entries: Vec<Entry>,
}

pub(super) struct Journal {
    dir: PathBuf,
    lock: File,
    pub entries: Vec<Entry>,
    failed: bool,
}

impl Drop for Journal {
    fn drop(&mut self) {
        // Forked children can briefly inherit this descriptor before close-on-exec.
        let _ = rustix::fs::flock(&self.lock, rustix::fs::FlockOperation::Unlock);
    }
}

impl Journal {
    pub fn open(home: &Home) -> Result<Self, Error> {
        home.ensure().map_err(|_| Error::Conflict)?;
        let dir = home.dir().join("source-work");
        match fs::DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => File::open(home.dir())?.sync_all()?,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => (),
            Err(error) => return Err(error.into()),
        }
        if !private_file::folder_private(&dir)? {
            return Err(Error::Conflict);
        }
        let lock = open(&dir.join("lock"), true)?;
        rustix::fs::flock(&lock, rustix::fs::FlockOperation::NonBlockingLockExclusive)
            .map_err(|_| Error::Busy)?;
        let entries = match open(&dir.join("journal.json"), false) {
            Ok(file) => {
                let bytes = private_file::read_limited(file, BYTES)?;
                let snapshot: Snapshot = serde_json::from_slice(&bytes)?;
                validate(&snapshot)?;
                snapshot.entries
            }
            Err(Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error),
        };
        Ok(Self {
            dir,
            lock,
            entries,
            failed: false,
        })
    }

    pub fn ready(&self) -> Result<(), Error> {
        if self.failed {
            Err(Error::Conflict)
        } else {
            Ok(())
        }
    }

    pub fn full(&self) -> bool {
        self.entries.len() >= LIMIT
    }

    pub fn forget(&mut self, execution: Uuid) -> Result<(), Error> {
        self.ready()?;
        let mut entries = self.entries.clone();
        let index = entries
            .iter()
            .position(|entry| entry.execution == execution && entry.settled)
            .ok_or(Error::Conflict)?;
        entries.remove(index);
        self.save(entries)
    }

    pub fn reserve(&mut self, device: Uuid, origin: &str, job: &WorkJob) -> Result<Entry, Error> {
        self.ready()?;
        validate_job(job)?;
        if let Some(entry) = self.entries.iter().find(|entry| {
            entry.device == device
                && entry.origin == origin
                && entry.job.id == job.id
                && entry.job.attempt == job.attempt
        }) {
            let mut retry = job.clone();
            retry.lease = entry.job.lease;
            if retry != entry.job || !entry.may_run() {
                return Err(Error::Conflict);
            }
            return Ok(entry.clone());
        }
        if self.full() {
            return Err(Error::Full);
        }
        let entry = Entry {
            execution: Uuid::now_v7(),
            device,
            origin: origin.to_owned(),
            job: job.clone(),
            report: None,
            settled: false,
            start_requested: false,
            stop_requested: false,
            checkout_requested_at: None,
            disposition: Disposition::Attached,
            renewal: None,
            reported: None,
            artifact_receipt: None,
        };
        let mut entries = self.entries.clone();
        entries.push(entry.clone());
        self.save(entries)?;
        Ok(entry)
    }

    pub fn replace(&mut self, entry: Entry) -> Result<(), Error> {
        self.ready()?;
        let mut entries = self.entries.clone();
        let old = entries
            .iter_mut()
            .find(|old| old.execution == entry.execution)
            .ok_or(Error::Conflict)?;
        if old.device != entry.device
            || old.origin != entry.origin
            || old.job != entry.job
            || (old.start_requested && !entry.start_requested)
            || (old.stop_requested && !entry.stop_requested)
            || old
                .checkout_requested_at
                .is_some_and(|requested| entry.checkout_requested_at != Some(requested))
            || (old.disposition == Disposition::Lost && entry.disposition != Disposition::Lost)
            || (old.settled && !entry.settled)
            || (old.artifact_receipt.is_some() && old.artifact_receipt != entry.artifact_receipt)
            || old
                .renewal
                .is_some_and(|grant| entry.renewal.is_none_or(|next| next.until < grant.until))
        {
            return Err(Error::Conflict);
        }
        *old = entry;
        self.save(entries)
    }

    fn save(&mut self, entries: Vec<Entry>) -> Result<(), Error> {
        let snapshot = Snapshot { schema: 1, entries };
        validate(&snapshot)?;
        let bytes = serde_json::to_vec(&snapshot)?;
        if bytes.len() > BYTES {
            return Err(Error::Full);
        }
        // A failed directory sync can leave the new identity on disk. Reopen before retrying.
        self.failed = true;
        private_file::write(&self.dir, "journal.json", &bytes)?;
        self.entries = snapshot.entries;
        self.failed = false;
        Ok(())
    }
}

fn open(path: &std::path::Path, create: bool) -> Result<File, Error> {
    use rustix::fs::{Mode, OFlags};
    let access = if create {
        OFlags::WRONLY | OFlags::CREATE
    } else {
        OFlags::RDONLY
    };
    let file = File::from(
        rustix::fs::open(
            path,
            access | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(std::io::Error::from)?,
    );
    let stat = file.metadata()?;
    if !stat.is_file()
        || stat.nlink() != 1
        || private_file::exposed(stat.mode(), stat.uid(), private_file::running_user()?)
    {
        return Err(Error::Conflict);
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_unlocks_even_while_a_child_descriptor_remains_open() {
        let temporary = tempfile::tempdir().unwrap();
        let home = Home::at(temporary.path().join("daemon"));
        let journal = Journal::open(&home).unwrap();
        let inherited = journal.lock.try_clone().unwrap();
        drop(journal);
        let reopened = Journal::open(&home).unwrap();
        assert!(matches!(Journal::open(&home), Err(Error::Busy)));
        drop(inherited);
        assert!(matches!(Journal::open(&home), Err(Error::Busy)));
        drop(reopened);
        assert!(Journal::open(&home).is_ok());
    }
}
