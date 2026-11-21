use super::schema::{Failure, Record};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

const RECORD_LIMIT: u64 = 16384;
pub const TERMINAL_LIMIT: usize = 256;
const RETENTION: u64 = 7 * 24 * 3600;

pub struct Journal {
    pub records: BTreeMap<Uuid, Record>,
    directory: PathBuf,
    _lock: File,
    failed: bool,
}

pub fn seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl Journal {
    pub fn open(path: &Path) -> Result<Self, Failure> {
        super::super::socket::directory(
            &path.join("record"),
            nix::unistd::Uid::effective().as_raw(),
        )
        .map_err(|_| Failure::JournalUnavailable)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(path.join("lock"))
            .map_err(|_| Failure::JournalUnavailable)?;
        lock.try_lock().map_err(|_| Failure::JournalUnavailable)?;
        let mut journal = Self {
            records: BTreeMap::new(),
            directory: path.into(),
            _lock: lock,
            failed: false,
        };
        journal.load().map_err(|_| Failure::JournalUnavailable)?;
        let pending: Vec<_> = journal
            .records
            .values()
            .filter(|r| !r.state.terminal())
            .cloned()
            .collect();
        for mut record in pending {
            record.state = super::schema::State::Interrupted;
            record.result = None;
            journal.save(record)?;
        }
        journal.prune()?;
        Ok(journal)
    }

    fn load(&mut self) -> std::io::Result<()> {
        for entry in fs::read_dir(&self.directory)? {
            let entry = entry?;
            let name = entry.file_name();
            if name == "lock" {
                continue;
            }
            if self.records.len() >= TERMINAL_LIMIT + 32 {
                return Err(std::io::Error::other("journal limit"));
            }
            let metadata = entry.metadata()?;
            if !entry.file_type()?.is_file()
                || metadata.permissions().mode() & 0o077 != 0
                || metadata.uid() != nix::unistd::Uid::effective().as_raw()
                || metadata.len() > RECORD_LIMIT
            {
                return Err(std::io::Error::other("invalid journal file"));
            }
            if name == "pending" {
                fs::remove_file(entry.path())?;
                continue;
            }
            let mut bytes = Vec::new();
            File::open(entry.path())?
                .take(RECORD_LIMIT + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() as u64 > RECORD_LIMIT {
                return Err(std::io::Error::other("record limit"));
            }
            let record: Record = serde_json::from_slice(&bytes)?;
            if name != format!("{}.json", record.id).as_str()
                || !valid(&record)
                || self
                    .records
                    .values()
                    .any(|r| r.request_id == record.request_id)
            {
                return Err(std::io::Error::other("invalid journal record"));
            }
            self.records.insert(record.id, record);
        }
        Ok(())
    }

    pub fn save(&mut self, mut record: Record) -> Result<Record, Failure> {
        if self.failed {
            return Err(Failure::JournalUnavailable);
        }
        record.revision = record
            .revision
            .checked_add(1)
            .ok_or(Failure::JournalUnavailable)?;
        record.updated_seconds = seconds();
        if self.write(&record).is_err() {
            self.failed = true;
            return Err(Failure::JournalUnavailable);
        }
        self.records.insert(record.id, record.clone());
        Ok(record)
    }

    fn write(&self, record: &Record) -> std::io::Result<()> {
        let bytes = serde_json::to_vec(record)?;
        if bytes.len() as u64 > RECORD_LIMIT {
            return Err(std::io::Error::other("record limit"));
        }
        let pending = self.directory.join("pending");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(&pending)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(pending, self.directory.join(format!("{}.json", record.id)))?;
        File::open(&self.directory)?.sync_all()
    }

    pub fn prune(&mut self) -> Result<(), Failure> {
        if self.failed {
            return Err(Failure::JournalUnavailable);
        }
        let now = seconds();
        let mut terminal: Vec<_> = self
            .records
            .values()
            .filter(|r| r.state.terminal())
            .cloned()
            .collect();
        terminal.sort_by_key(|r| (r.updated_seconds, r.id));
        let excess = terminal.len().saturating_sub(TERMINAL_LIMIT);
        for (index, record) in terminal.into_iter().enumerate() {
            if index < excess || now.saturating_sub(record.updated_seconds) > RETENTION {
                if fs::remove_file(self.directory.join(format!("{}.json", record.id)))
                    .and_then(|()| File::open(&self.directory)?.sync_all())
                    .is_err()
                {
                    self.failed = true;
                    return Err(Failure::JournalUnavailable);
                }
                self.records.remove(&record.id);
            }
        }
        Ok(())
    }
}

fn valid(record: &Record) -> bool {
    use super::schema::State;
    record.revision > 0
        && record.revision < u64::MAX
        && !record.id.is_nil()
        && !record.request_id.is_nil()
        && Uuid::parse_str(&record.boot_id).is_ok()
        && ((record.state == State::Succeeded) == record.result.is_some())
        && (record.state != State::Cancelled || record.cancel_requested)
        && (record.state != State::Succeeded || !record.cancel_requested)
}
