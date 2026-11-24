//! `runtime_install` (Section 6, CAG-008): no shell, no install script, no package manager.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::folders::Folders;
use daemon_core::model_work::names::Word as _;
use daemon_core::model_work::releases::fetch::{FetchOrder, child_deadline};
use daemon_core::model_work::releases::{self, Release, SourceEntry, Sources};

use super::{fetch, folders, unpack};

pub const DEFAULT_RESERVE: u64 = 10 << 30;

pub struct Place<'a> {
    pub folders: &'a Folders,
    pub sources: &'a Sources,
    pub os: &'a str,
    pub architecture: &'a str,
    pub reserve: u64,
    /// The daemon program, which runs the child fetch process.
    pub program: &'a Path,
}

#[derive(Clone)]
pub struct Target {
    pub folders: Folders,
    pub release: Release,
    pub entry: SourceEntry,
    pub reserve: u64,
}

#[must_use]
pub fn installed(folders: &Folders, release: &Release, entry: &SourceEntry) -> bool {
    let program = folders.release(release).join(&entry.program);
    std::fs::symlink_metadata(program).is_ok_and(|meta| meta.is_file())
}

fn fits(path: &Path, need: u64, reserve: u64) -> bool {
    folders::free_bytes(path).is_ok_and(|free| free >= need.saturating_add(reserve))
}

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

/// Unpacks a verified `archive` and renames its folder into place. Blocking.
///
/// # Errors
/// As [`unpack::unpack`]; `archive_rejected` when the archive has no program at its entry's path.
pub fn place_archive(archive: std::fs::File, target: &Target) -> Result<(), Cause> {
    let temporary = unpack_folder(target)?;
    let parent: PathBuf = temporary.path().to_owned();
    let holds = || fits(&parent, 0, target.reserve);
    let format = target.entry.format().ok_or(Cause::RuntimeNotConfigured)?;
    unpack::unpack(archive, format, temporary.path(), holds)?;
    settle(temporary, target)
}

fn unpack_folder(target: &Target) -> Result<TempDir, Cause> {
    use std::os::unix::fs::PermissionsExt as _;
    let runtime = target.release.runtime.as_str();
    folders::private_below(&target.folders.runtimes, Path::new(runtime))
        .map_err(|_| Cause::LimitDisk)?;
    let temporary = tempfile::Builder::new()
        .prefix(".unpack-")
        .tempdir_in(target.folders.runtimes.join(runtime))
        .map_err(|_| Cause::LimitDisk)?;
    // The folder becomes `<version>-<backend>/`; its mode must not follow the umask (Section 6).
    std::fs::set_permissions(temporary.path(), std::fs::Permissions::from_mode(0o755))
        .map_err(|_| Cause::LimitDisk)?;
    Ok(temporary)
}

fn settle(temporary: TempDir, target: &Target) -> Result<(), Cause> {
    let program = temporary.path().join(&target.entry.program);
    if !std::fs::symlink_metadata(&program).is_ok_and(|meta| meta.is_file()) {
        return Err(Cause::ArchiveRejected);
    }
    std::fs::rename(temporary.path(), target.folders.release(&target.release))
        .map_err(|_| Cause::LimitDisk)?;
    let _ = temporary.keep();
    Ok(())
}
