//! Found models at the daemon: the walk of the folders of Section 10.2, `copy_adopt`, and the
//! check of a found copy at each load (`contracts/compute/model-work.md` Section 2.3; PULL-012,
//! R-444). The daemon never writes, renames, deletes, locks or changes the mode of anything in
//! those folders.

pub mod open;
pub mod record;
pub mod walk;

use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::found::record::{Observed, Record, RecordFile, named_hash_holds};
use daemon_core::model_work::found::{FoundModel, Root};
use serde_json::Value;
use uuid::Uuid;

use open::{Folder, Opened, size_and_sha256};

/// The list `found` of the result of `model_report` (Section 8), walked on a blocking thread.
pub async fn listed(roots: Vec<Root>) -> Vec<Value> {
    let walk = move || walk::walk(&roots).iter().map(FoundModel::to_json).collect();
    tokio::task::spawn_blocking(walk).await.unwrap_or_default()
}

/// [`adopt_now`] on a blocking thread: it reads each file of the model.
///
/// # Errors
/// As [`adopt_now`].
pub async fn adopt(
    roots: Vec<Root>,
    state: PathBuf,
    copy: Uuid,
    found: String,
) -> Result<Value, Cause> {
    let adopt = move || adopt_now(&roots, &state, copy, &found);
    tokio::task::spawn_blocking(adopt)
        .await
        .map_err(|_| Cause::CopyMissing)?
}

/// The file at `location` below the folder of Section 10.2, opened by the layout rules.
fn open_at(root: &Folder, location: &[String]) -> Result<Opened, Cause> {
    let (name, folders) = location.split_last().ok_or(Cause::CopyMissing)?;
    if folders.is_empty() {
        return root.file(name);
    }
    root.below(folders.iter().map(String::as_str))
        .ok_or(Cause::CopyMissing)?
        .file(name)
}

/// `copy_adopt`: finds the model with the key `found` again, reads each of its files and writes
/// the record of `copy`. Gives the `model` member of the result.
///
/// # Errors
/// `found_unknown` when no walk finds the key; `file_type` for a model that is not GGUF
/// (MW-P10); `copy_missing` for a file that is gone; `checksum` when a file is not the SHA-256
/// that the tool names it by; `limit_disk` when the record cannot be written.
pub fn adopt_now(roots: &[Root], state: &Path, copy: Uuid, found: &str) -> Result<Value, Cause> {
    let model = walk::walk(roots)
        .into_iter()
        .find(|model| model.key() == found)
        .ok_or(Cause::FoundUnknown)?;
    if !model.usable {
        return Err(Cause::FileType);
    }
    let root = Folder::root(Path::new(&model.root)).ok_or(Cause::CopyMissing)?;
    let mut files = Vec::with_capacity(model.files.len());
    for file in &model.files {
        let opened = open_at(&root, &file.location)?;
        let (size_bytes, sha256) = size_and_sha256(&opened.fd).map_err(|_| Cause::CopyMissing)?;
        files.push(RecordFile {
            location: file.location.clone(),
            size_bytes,
            sha256,
            device: opened.meta.dev(),
            inode: opened.meta.ino(),
            modified: opened.meta.mtime(),
        });
    }
    let hashes: Vec<String> = files.iter().map(|file| file.sha256.clone()).collect();
    named_hash_holds(&model, &hashes)?;
    let record = Record {
        copy,
        found: found.to_owned(),
        tool: model.tool,
        root: model.root,
        files,
    };
    record::write(state, &record)?;
    Ok(record.result())
}

/// The check at each load of a found copy: each file of the record, open, with the size and the
/// SHA-256 of the record. The runtime gets these descriptors, never a path (Section 2.3).
///
/// # Errors
/// `copy_unknown` with no record; `copy_missing` for a file that is gone or not a regular file,
/// or a folder that the walk no longer reads; `copy_changed` for a different size or SHA-256.
pub fn check(roots: &[Root], state: &Path, copy: Uuid) -> Result<Vec<OwnedFd>, Cause> {
    let record = record::read(state, copy)?;
    let read = roots
        .iter()
        .any(|root| root.tool == record.tool && root.path.to_str() == Some(record.root.as_str()));
    if !read {
        return Err(Cause::CopyMissing);
    }
    let root = Folder::root(Path::new(&record.root)).ok_or(Cause::CopyMissing)?;
    record
        .files
        .iter()
        .map(|file| {
            let opened = open_at(&root, &file.location)?;
            let (size_bytes, sha256) =
                size_and_sha256(&opened.fd).map_err(|_| Cause::CopyMissing)?;
            file.holds(Observed {
                size_bytes,
                sha256: &sha256,
            })?;
            Ok(opened.fd)
        })
        .collect()
}
