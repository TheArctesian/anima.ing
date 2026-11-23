//! The record of the holder: for each running program, its thread, its process ID and the start
//! time of the process, in a file of mode 0600 of the user (H9). A program that ended and left a
//! process that its sweep could not prove ended stays in the record with the processes seen below
//! it (THR-049). A holder that starts ends each process an older holder left, if the start time
//! is the same, so that it never ends a process that took an old ID; it sweeps again what an
//! ended program left, also by the mark of its thread.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::tree;
use super::wire::ThreadKey;
use crate::config::private_file;

const FILE: &str = "holder.json";

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub version: u32,
    pub threads: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub thread: ThreadKey,
    pub pid: u32,
    pub started: Option<String>,
    /// The program ended, and a process it left may still run: the processes seen below it,
    /// each with its start time. Absent in the record of an older holder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<tree::Known>,
}

/// Writes the record in place of the old one.
///
/// # Errors
/// The error of the file when the record cannot be written.
pub fn write(dir: &Path, record: &Record) -> Result<(), private_file::PrivateFileError> {
    let bytes = serde_json::to_vec(record).unwrap_or_default();
    private_file::write(dir, FILE, &bytes)
}

/// The record that is in `dir`, if one is there and reads.
#[must_use]
pub fn read(dir: &Path) -> Option<Record> {
    let bytes = private_file::read(&dir.join(FILE)).ok()??;
    serde_json::from_slice(&bytes).ok()
}

/// Ends each program that an older holder recorded and that still runs with the same start time,
/// with the processes below it, and sweeps again what an ended program left. Gives the entries
/// whose end it could not prove, and keeps only them in the record, so that the next start of a
/// holder tries them again.
pub async fn end_left_over(dir: &Path) -> Vec<Entry> {
    let Some(old) = read(dir) else {
        return Vec::new();
    };
    let mut kept = Vec::new();
    for mut entry in old.threads {
        if !ended(&entry).await {
            // A program whose end is not proved may be gone at the next start while what it
            // left still runs: from then on its thread is swept by its mark.
            entry.left.get_or_insert_with(tree::Known::new);
            kept.push(entry);
        }
    }
    let _ = write(
        dir,
        &Record {
            version: old.version,
            threads: kept.clone(),
        },
    );
    kept
}

/// Whether the end of what `entry` names is proved after this holder ended it: its program by
/// its start time, while it still runs as the same process; then what it left, by start time
/// and by the mark of its thread.
async fn ended(entry: &Entry) -> bool {
    let same = entry.started.is_some() && tree::started(entry.pid).await == entry.started;
    let program = !same
        || tree::end(entry.pid, true, &tree::Known::new(), Duration::from_secs(3))
            .await
            .all;
    let Some(left) = &entry.left else {
        return program;
    };
    let swept = tree::kill_seen(left).await;
    let marked = super::marker::kill_marked(&entry.thread, &super::marker::Keep::nothing()).await;
    program && swept.and(marked).settled().await.all
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thr_049_a_leftover_stays_in_the_record_and_an_older_record_still_reads() {
        let older = r#"{"version":2,"threads":[{"thread":"a","pid":41,"started":"Sat Nov 21 10:00:00 2026"}]}"#;
        let read: Record = serde_json::from_str(older).expect("an older record reads");
        assert_eq!(
            read.threads[0].left, None,
            "a running program of an older holder"
        );

        let left = tree::Known::from([(42, "Sat Nov 21 10:00:01 2026".to_owned())]);
        let record = Record {
            version: 2,
            threads: vec![Entry {
                thread: "b".into(),
                pid: 41,
                started: None,
                left: Some(left.clone()),
            }],
        };
        let again: Record =
            serde_json::from_slice(&serde_json::to_vec(&record).expect("written")).expect("read");
        assert_eq!(again.threads[0].left, Some(left));
    }
}
