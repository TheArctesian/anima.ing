//! Persist the first hub pair and any mismatch before answering a request.
use std::{io, path::Path};

use crate::{
    Pair, PairDecision, PairFile, PairRecord, Refusal,
    execution::{Fault, files::Store},
    pair_decided,
};

pub(super) struct Pairing<'a> {
    store: Store<'a>,
    path: String,
    write_failed: bool,
}

impl<'a> Pairing<'a> {
    pub(super) fn open(path: &'a Path) -> Result<Self, Fault> {
        let store = Store::open(path)?;
        let pair = path.join("hub-pair.json");
        let path = pair
            .into_os_string()
            .into_string()
            .map_err(|_| Fault::UnsafePlan)?;
        Ok(Self {
            store,
            path,
            write_failed: false,
        })
    }

    pub(super) fn accept(&mut self, set_on_at: u64, pair: &Pair) -> Result<(), Refusal> {
        if self.write_failed {
            return Err(Refusal::ApplicationsOff);
        }
        let record = self.store.read_small_file(&self.path, 1024);
        let parsed = record
            .as_ref()
            .ok()
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .and_then(PairRecord::read);
        let found = match (record, parsed.as_ref()) {
            (Err(Fault::Storage(io::ErrorKind::NotFound)), _) => PairFile::Missing,
            (_, Some(record)) => PairFile::Read(record),
            _ => PairFile::Unreadable,
        };
        match pair_decided(found, set_on_at, pair) {
            PairDecision::Recorded => Ok(()),
            PairDecision::First(record) => self.write(&record),
            PairDecision::Different(record) => {
                if let Some(record) = record {
                    self.write(&record)?;
                }
                Err(Refusal::ApplicationsOff)
            }
        }
    }

    fn write(&mut self, record: &PairRecord) -> Result<(), Refusal> {
        if self
            .store
            .write(&self.path, record.written().as_bytes())
            .is_err()
        {
            self.write_failed = true;
            return Err(Refusal::ApplicationsOff);
        }
        Ok(())
    }
}
