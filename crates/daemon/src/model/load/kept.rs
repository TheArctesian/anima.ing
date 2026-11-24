//! The keep files `<model-state>/keep/<copy>.json` (K11 Section 11.1): mode 0600, at most 4 KiB, written as a
//! temporary file and moved into place, at most 64, and checked when read.

use std::collections::BTreeMap;
use std::path::Path;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::memory::keep::{Kept, MAX_FILE_BYTES, MAX_FILES};
use uuid::Uuid;

use crate::model::folders::{private_below, read_private, remove_private, write_private};

const FOLDER: &str = "keep";

fn name_of(copy: Uuid) -> String {
    format!("{copy}.json")
}

/// The keep files of the node. A file that fails a check is deleted. A pinned copy that was on a server before
/// the restart is marked `displaced`, so that it returns by the rules (the daemon loads nothing at its start).
pub fn open(state: &Path) -> BTreeMap<Uuid, Kept> {
    let mut found = BTreeMap::new();
    // The folder is checked (private, of this user, no link) before it is listed.
    if private_below(state, Path::new(FOLDER)).is_err() {
        return found;
    }
    let Ok(entries) = std::fs::read_dir(state.join(FOLDER)) else {
        return found;
    };
    let names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    for name in names {
        let Some(copy) = name
            .strip_suffix(".json")
            .and_then(|id| Uuid::parse_str(id).ok())
        else {
            continue;
        };
        let read = read_private(state, FOLDER, &name, MAX_FILE_BYTES)
            .ok()
            .and_then(|bytes| Kept::read(&bytes));
        match read {
            Some(mut kept) if found.len() < MAX_FILES => {
                if kept.keep.pinned && kept.server.is_some() && !kept.displaced {
                    kept.displaced = true;
                    let _ = write(state, copy, &kept);
                }
                found.insert(copy, kept);
            }
            _ => {
                let _ = remove_private(state, FOLDER, &name);
            }
        }
    }
    found
}

/// # Errors
/// `limit_disk` when the state folder cannot be written.
pub fn write(state: &Path, copy: Uuid, kept: &Kept) -> Result<(), Cause> {
    let text = serde_json::to_vec(kept).map_err(|_| Cause::LimitDisk)?;
    if u64::try_from(text.len()).map_or(true, |length| length > MAX_FILE_BYTES) {
        return Err(Cause::MemberUnknown);
    }
    write_private(state, FOLDER, &name_of(copy), &text).map_err(|_| Cause::LimitDisk)
}

/// # Errors
/// `limit_disk` when the state folder cannot be written.
pub fn forget(state: &Path, copy: Uuid) -> Result<(), Cause> {
    remove_private(state, FOLDER, &name_of(copy)).map_err(|_| Cause::LimitDisk)
}
