//! The records of the found copies in `<model-state>/found/` (Section 2.3, Section 10): written
//! with `O_EXCL` and mode 0600, and read back as untrusted.

use std::path::Path;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::found::record::Record;
use uuid::Uuid;

use crate::model::folders::{read_private, remove_private, write_private};

/// The folder of the records in the state folder.
const FOLDER: &str = "found";

/// The most bytes of one record.
const MAX_RECORD: u64 = 1 << 20;

fn name_of(copy: Uuid) -> String {
    format!("{copy}.json")
}

/// Writes the record to a new file and moves it over the record of an earlier attempt.
///
/// # Errors
/// `limit_disk` when the state folder cannot be written.
pub fn write(state: &Path, record: &Record) -> Result<(), Cause> {
    let text = serde_json::to_vec(record).map_err(|_| Cause::LimitDisk)?;
    write_private(state, FOLDER, &name_of(record.copy), &text).map_err(|_| Cause::LimitDisk)
}

/// The record of `copy`.
///
/// # Errors
/// `copy_unknown` when there is none, or it is not a plain file of this user of the form that
/// the daemon writes.
pub fn read(state: &Path, copy: Uuid) -> Result<Record, Cause> {
    let text =
        read_private(state, FOLDER, &name_of(copy), MAX_RECORD).map_err(|_| Cause::CopyUnknown)?;
    let record: Record = serde_json::from_slice(&text).map_err(|_| Cause::CopyUnknown)?;
    (record.copy == copy && record.holds_form())
        .then_some(record)
        .ok_or(Cause::CopyUnknown)
}

/// Deletes the record of `copy`, and only the record: no file of the found model (Section 2.3).
/// A record that is not there is no error.
///
/// # Errors
/// `limit_disk` when the state folder cannot be written.
pub fn forget(state: &Path, copy: Uuid) -> Result<(), Cause> {
    remove_private(state, FOLDER, &name_of(copy)).map_err(|_| Cause::LimitDisk)
}
