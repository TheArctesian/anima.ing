//! `model_pull` and `copy_delete` at the daemon (Section 7, PULL-004 to PULL-009): the checks
//! before the start, the pull process in [`child::PULL_MODE`], and the removal of a copy.

pub mod child;
pub mod partial;
pub mod watch;

use std::ffi::CString;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::time::SystemTime;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::machine::MachineSettings;
use daemon_core::model_work::names::Word as _;
use daemon_core::model_work::order::Parameters;
use daemon_core::model_work::pull::{
    PULL_CAUSES, PullOrder, child_deadline, fits, partial_room, stale, total_bytes,
};
use daemon_core::model_work::releases::fetch::verdict_in;
use serde_json::{Value, json};
use uuid::Uuid;

use super::fetch::spawn::run_child;
use super::folders;

/// What a pull needs of the daemon.
pub struct Cache<'a> {
    pub models: &'a Path,
    /// The free disk that a pull leaves (`limit_disk` below it).
    pub reserve: u64,
    /// The daemon program, started again in [`child::PULL_MODE`].
    pub program: &'a Path,
    pub settings: &'a MachineSettings,
}

fn below(models: &Path, folder: &str) -> Result<OwnedFd, Cause> {
    folders::private_below(models, Path::new(folder)).map_err(|_| Cause::LimitDisk)
}

fn name_of(copy: Uuid) -> CString {
    CString::new(copy.to_string()).unwrap_or_default()
}

/// The `model` member of the result: the copy is `downloaded`, never `validated` (PULL-011).
fn downloaded(copy: Uuid, bytes: u64, files: usize) -> Value {
    json!({ "copy": copy, "copyState": "downloaded", "bytes": bytes, "files": files })
}

/// # Errors
/// The cause of Section 5: a refusal before the start, or the cause of the pull process.
pub async fn pull(parameters: &Parameters, cache: &Cache<'_>) -> Result<Option<Value>, Cause> {
    let Parameters::Pull {
        copy,
        source,
        format,
        files,
        gated,
        ..
    } = parameters
    else {
        return Err(Cause::TypeUnknown);
    };
    refuse_unbuilt(*gated, cache.settings)?;
    let total = total_bytes(files);
    let name = name_of(*copy);
    if present(&below(cache.models, "copies")?, &name)? {
        return Ok(Some(downloaded(*copy, total, files.len())));
    }
    let have = make_room(&below(cache.models, "partial")?, &name)?;
    let free = folders::free_bytes(cache.models).map_err(|_| Cause::LimitDisk)?;
    if !fits(free, total, have, cache.reserve) {
        return Err(Cause::DiskShare);
    }
    let order = PullOrder {
        copy: *copy,
        source: source.clone(),
        format: format.as_str().to_owned(),
        files: files.clone(),
        // A path that is not UTF-8 would name a different folder in the process.
        models: cache.models.to_str().ok_or(Cause::LimitDisk)?.to_owned(),
        reserve: cache.reserve,
        parent: std::process::id(),
    };
    let text = serde_json::to_vec(&order).map_err(|_| Cause::FetchFailed)?;
    let ended = run_child(cache.program, child::PULL_MODE, text, child_deadline(total)).await;
    verdict_in(ended.ending(), &PULL_CAUSES)?;
    Ok(Some(downloaded(*copy, total, files.len())))
}

/// The parts of Section 7 that this build does not do refuse the pull, never skip a check.
fn refuse_unbuilt(gated: bool, settings: &MachineSettings) -> Result<(), Cause> {
    // TODO(L1-7 follow-up): the secret `hf_token` of K10 Section 8 is not built (PULL-010).
    if gated {
        return Err(Cause::SecretUnknown);
    }
    // TODO(L1-7 follow-up): the second hash and the age of a revision (R-339) are not built.
    if settings.second_hash {
        return Err(Cause::HashMismatch);
    }
    if settings.revision_age_days > 0 {
        return Err(Cause::RevisionTooNew);
    }
    Ok(())
}

/// Whether `copies/` holds the folder of the copy: the pull process moves it there only when
/// each file was verified. Any other entry of that name is `limit_disk`, never a copy.
fn present(copies: &OwnedFd, name: &CString) -> Result<bool, Cause> {
    partial::own_folder(copies, name).map_err(|_| Cause::LimitDisk)
}

/// Deletes each other partial copy that no pull touched for 7 days, then refuses a fifth one
/// (`disk_share`). Gives the bytes that the partial copy of this pull holds already.
fn make_room(partial: &OwnedFd, own: &CString) -> Result<u64, Cause> {
    let found =
        partial::partial_copies(partial, SystemTime::now()).map_err(|_| Cause::LimitDisk)?;
    let mut others = 0;
    let mut have = None;
    for entry in found {
        if entry.name == *own {
            have = Some(entry.bytes);
        } else if stale(entry.age) {
            partial::remove_below(partial, &entry.name, 0).map_err(|_| Cause::LimitDisk)?;
        } else {
            others += 1;
        }
    }
    if !partial_room(others, have.is_some()) {
        return Err(Cause::DiskShare);
    }
    Ok(have.unwrap_or(0))
}

/// `copy_delete` (PULL-009): removes `copies/<copy>` and `partial/<copy>`; no link is followed
/// (Section 10). A copy that is not there is no error.
///
/// # Errors
/// `limit_disk` when the cache cannot be changed.
pub fn delete(models: &Path, copy: Uuid) -> Result<Option<Value>, Cause> {
    let name = name_of(copy);
    for folder in ["copies", "partial"] {
        partial::remove_below(&below(models, folder)?, &name, 0).map_err(|_| Cause::LimitDisk)?;
    }
    Ok(None)
}

/// The bytes that the copy holds now, in `partial/` and `copies/`, read relative to each
/// folder with no link followed (Section 10); 0 for each folder that cannot be read.
#[must_use]
pub fn bytes_of(models: &Path, copy: Uuid) -> u64 {
    let name = name_of(copy);
    ["partial", "copies"]
        .into_iter()
        .filter_map(|folder| below(models, folder).ok())
        .map(|folder| partial::bytes_in(&folder, &name))
        .sum()
}
