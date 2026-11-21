//! `runtime_install` (Section 6, CAG-008): the release source of the configuration, a check of
//! the free disk against the reserve, then the child fetch process ([`fetch::spawn`]), which
//! downloads in a total time, verifies the SHA-256 before it opens the archive and unpacks into a
//! new temporary folder; then the check of the program and one rename to
//! `runtimes/<runtime>/<version>-<backend>/`. No shell, no install script, no package manager.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::folders::Folders;
use daemon_core::model_work::names::Word as _;
use daemon_core::model_work::releases::fetch::{FetchOrder, child_deadline};
use daemon_core::model_work::releases::{self, Release, SourceEntry, Sources};

use super::{fetch, folders, unpack};

/// The free disk that model work leaves to the machine (Section 7; default 10 GiB).
pub const DEFAULT_RESERVE: u64 = 10 << 30;

/// Where the machine is and what its operator configured.
pub struct Place<'a> {
    pub folders: &'a Folders,
    pub sources: &'a Sources,
    pub os: &'a str,
    pub architecture: &'a str,
    /// The disk reserve of the daemon configuration.
    pub reserve: u64,
    /// The daemon program, which runs the child fetch process.
    pub program: &'a Path,
}

/// What the unpack of one archive writes, and the reserve that it keeps.
#[derive(Clone)]
pub struct Target {
    pub folders: Folders,
    pub release: Release,
    pub entry: SourceEntry,
    pub reserve: u64,
}

/// Whether `release` is installed: its folder holds the program of its entry as a regular file.
#[must_use]
pub fn installed(folders: &Folders, release: &Release, entry: &SourceEntry) -> bool {
    let program = folders.release(release).join(&entry.program);
    std::fs::symlink_metadata(program).is_ok_and(|meta| meta.is_file())
}

/// Whether the file system of `path` has `need` bytes free above `reserve`.
fn fits(path: &Path, need: u64, reserve: u64) -> bool {
    folders::free_bytes(path).is_ok_and(|free| free >= need.saturating_add(reserve))
}

/// Installs `release`.
///
/// # Errors
/// The cause of Section 5 for the first step that fails.
pub async fn install(release: &Release, place: &Place<'_>) -> Result<(), Cause> {
    releases::accepted(release)?;
    let entry = place.sources.find(release, place.os, place.architecture)?;
    if installed(place.folders, release, entry) {
        return Ok(());
    }
    folders::make_all(place.folders).map_err(|_| Cause::LimitDisk)?;
    let roots = [&place.folders.state, &place.folders.runtimes];
    if !roots
        .iter()
        .all(|root| fits(root, entry.size_bytes, place.reserve))
    {
        return Err(Cause::LimitDisk);
    }
    let target = Target {
        folders: place.folders.clone(),
        release: release.clone(),
        entry: entry.clone(),
        reserve: place.reserve,
    };
    let temporary = unpack_folder(&target)?;
    let order = order(&target, temporary.path())?;
    fetch::spawn::fetch_in_child(place.program, &order, child_deadline(entry.size_bytes)).await?;
    tokio::task::spawn_blocking(move || settle(temporary, &target))
        .await
        .map_err(|_| Cause::LimitDisk)?
}

/// The order of the child for `target`, unpacked into `into`.
fn order(target: &Target, into: &Path) -> Result<FetchOrder, Cause> {
    let text = |path: &Path| path.to_str().map(str::to_owned).ok_or(Cause::LimitDisk);
    Ok(FetchOrder {
        runtime: target.release.runtime.as_str().to_owned(),
        address: target.entry.address.clone(),
        sha256: target.entry.sha256.clone(),
        size_bytes: target.entry.size_bytes,
        archive_folder: text(&target.folders.state)?,
        unpack_folder: text(into)?,
        reserve: target.reserve,
        parent: std::process::id(),
    })
}

/// Unpacks a verified `archive` in this process and renames its folder into place. Blocking.
///
/// # Errors
/// As [`unpack::unpack`]; `archive_rejected` when the archive has no program at the path of
/// its entry.
pub fn place_archive(archive: std::fs::File, target: &Target) -> Result<(), Cause> {
    let temporary = unpack_folder(target)?;
    let parent: PathBuf = temporary.path().to_owned();
    let holds = || fits(&parent, 0, target.reserve);
    let format = target.entry.format().ok_or(Cause::RuntimeNotConfigured)?;
    unpack::unpack(archive, format, temporary.path(), holds)?;
    settle(temporary, target)
}

/// A new empty folder below `runtimes/<runtime>/`, deleted when it is dropped.
fn unpack_folder(target: &Target) -> Result<TempDir, Cause> {
    let runtime = target.release.runtime.as_str();
    folders::private_below(&target.folders.runtimes, Path::new(runtime))
        .map_err(|_| Cause::LimitDisk)?;
    tempfile::Builder::new()
        .prefix(".unpack-")
        .tempdir_in(target.folders.runtimes.join(runtime))
        .map_err(|_| Cause::LimitDisk)
}

/// Checks the program of the unpacked `temporary` folder and renames it into place. Blocking.
///
/// # Errors
/// `archive_rejected` when there is no program at the path of the entry.
fn settle(temporary: TempDir, target: &Target) -> Result<(), Cause> {
    let program = temporary.path().join(&target.entry.program);
    if !std::fs::symlink_metadata(&program).is_ok_and(|meta| meta.is_file()) {
        return Err(Cause::ArchiveRejected);
    }
    std::fs::rename(temporary.path(), target.folders.release(&target.release))
        .map_err(|_| Cause::LimitDisk)?;
    // The folder now has its final name; nothing is left for the guard to delete.
    let _ = temporary.keep();
    Ok(())
}
