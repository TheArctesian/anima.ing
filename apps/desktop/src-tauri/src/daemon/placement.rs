//! Places the daemon binary in `~/.local/bin`, the folder of the page's install command, so both ways replace one file (MET-019, D-10 d).
//! The copy is atomic and refused when any folder on the way can be changed by another user ([`super::trust`]).

use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
};

use super::trust::{self, Stat, User};

/// The bundle names the sidecar `binaries/anima-daemon` (HMD-052); Tauri drops the target triple and puts it beside the app.
pub const DAEMON_FILE: &str = "anima-daemon";

const BINARY_MODE: u32 = 0o755;
const FOLDER_MODE: u32 = 0o755;
const SIZE_LIMIT: u64 = 256 * 1024 * 1024;

/// Fixed text: no path and no OS error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlacementError {
    #[error("a folder on the way to the daemon can be changed by another user")]
    Untrusted,
    #[error("the bundled daemon is not a regular file of a sane size")]
    Source,
    #[error("a folder could not be made, or the daemon could not be written or renamed")]
    Write,
    #[error("the new copy of the daemon did not pass its check")]
    Refused,
}

pub fn file_trusted(file: &Path, by: User) -> bool {
    fs::symlink_metadata(file).is_ok_and(|metadata| {
        let stat = Stat {
            mode: metadata.mode(),
            owner: metadata.uid(),
            group: metadata.gid(),
            acl: has_acl(file).unwrap_or(true),
        };
        metadata.is_file() && trust::folder_trusted(stat, by)
    })
}

/// The real path of `dir`, made when missing, after the checks of each folder above it.
///
/// # Errors
/// [`PlacementError::Untrusted`] or [`PlacementError::Write`].
pub fn trusted_folder(dir: &Path, by: User) -> Result<PathBuf, PlacementError> {
    make_folder(dir, by)
}

pub fn destination(home: &Path) -> PathBuf {
    home.join(".local/bin").join(DAEMON_FILE)
}

pub fn bundled_beside(executable: &Path) -> Option<PathBuf> {
    executable.parent().map(|folder| folder.join(DAEMON_FILE))
}

/// Copies `source` under a fresh name into a checked folder and renames it to `destination`; returns the real path that was checked and is to be run.
///
/// # Errors
/// [`PlacementError`] for the first step that fails; nothing is left at the destination then.
pub fn place(source: &Path, destination: &Path, by: User) -> Result<PathBuf, PlacementError> {
    place_checked(source, destination, by, &|_| true)
}

/// As [`place`], renaming only when `check` accepts the new file: the bytes that will run, not the source, which can change.
///
/// # Errors
/// As [`place`]; [`PlacementError::Refused`] when `check` refused the new file.
pub fn place_checked(
    source: &Path,
    destination: &Path,
    by: User,
    check: &dyn Fn(&Path) -> bool,
) -> Result<PathBuf, PlacementError> {
    let folder = make_folder(destination.parent().ok_or(PlacementError::Write)?, by)?;
    let mut input = open_source(source)?;
    let name = destination.file_name().ok_or(PlacementError::Write)?;
    let part = folder.join(part_name()?);
    let installed = folder.join(name);
    let refused = |error: PlacementError| {
        let _ = fs::remove_file(&part);
        error
    };
    copy_to(&mut input, &part).map_err(|_| refused(PlacementError::Write))?;
    if !check(&part) {
        return Err(refused(PlacementError::Refused));
    }
    fs::rename(&part, &installed)
        .and_then(|()| File::open(&folder)?.sync_all())
        .map_err(|_| refused(PlacementError::Write))?;
    Ok(installed)
}

/// Each existing and each new part is resolved and checked with every folder above it, so no folder is made through a link to a folder another user may write.
fn make_folder(dir: &Path, by: User) -> Result<PathBuf, PlacementError> {
    let existing = dir
        .ancestors()
        .find(|folder| fs::metadata(folder).is_ok())
        .unwrap_or_else(|| Path::new("/"));
    let mut real = existing.canonicalize().map_err(|_| PlacementError::Write)?;
    check_chain(&real, by)?;
    let missing = dir.strip_prefix(existing).unwrap_or_else(|_| Path::new(""));
    for part in missing.components() {
        real.push(part);
        match DirBuilder::new().mode(FOLDER_MODE).create(&real) {
            Err(error) if error.kind() != io::ErrorKind::AlreadyExists => {
                return Err(PlacementError::Write);
            }
            _ => {}
        }
        real = real.canonicalize().map_err(|_| PlacementError::Write)?;
        check_chain(&real, by)?;
    }
    Ok(real)
}

fn check_chain(folder: &Path, by: User) -> Result<(), PlacementError> {
    for each in folder.ancestors() {
        let metadata = fs::symlink_metadata(each).map_err(|_| PlacementError::Untrusted)?;
        let stat = Stat {
            mode: metadata.mode(),
            owner: metadata.uid(),
            group: metadata.gid(),
            acl: has_acl(each)?,
        };
        if !metadata.is_dir() || !trust::folder_trusted(stat, by) {
            return Err(PlacementError::Untrusted);
        }
    }
    Ok(())
}

/// An ACL that cannot be read is not trusted; NFS version 4, CIFS and ZFS native ACLs are not seen.
#[cfg(target_os = "linux")]
fn has_acl(path: &Path) -> Result<bool, PlacementError> {
    use rustix::io::Errno;
    match rustix::fs::getxattr_query(path, "system.posix_acl_access") {
        Ok(_) => Ok(true),
        Err(Errno::NODATA | Errno::NOTSUP) => Ok(false),
        Err(_) => Err(PlacementError::Untrusted),
    }
}

/// macOS ACLs (`chmod +a`) do not show in the mode and are not read.
#[cfg(not(target_os = "linux"))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the same signature as the Linux reader"
)]
fn has_acl(_path: &Path) -> Result<bool, PlacementError> {
    Ok(false)
}

fn open_source(source: &Path) -> Result<File, PlacementError> {
    let linked = fs::symlink_metadata(source).map_err(|_| PlacementError::Source)?;
    let file = File::open(source).map_err(|_| PlacementError::Source)?;
    let opened = file.metadata().map_err(|_| PlacementError::Source)?;
    let regular = linked.is_file() && opened.is_file() && linked.ino() == opened.ino();
    if !regular || opened.len() > SIZE_LIMIT {
        return Err(PlacementError::Source);
    }
    Ok(file)
}

fn part_name() -> Result<String, PlacementError> {
    let mut salt = [0_u8; 8];
    getrandom::fill(&mut salt).map_err(|_| PlacementError::Write)?;
    Ok(format!(
        ".{DAEMON_FILE}.{:016x}.part",
        u64::from_le_bytes(salt)
    ))
}

fn copy_to(input: &mut File, part: &Path) -> io::Result<()> {
    // `create_new` is O_CREAT|O_EXCL: an existing file or a planted link is refused, not followed.
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(part)?;
    let copied = io::copy(&mut input.take(SIZE_LIMIT + 1), &mut output)?;
    if copied > SIZE_LIMIT {
        return Err(io::Error::other("the daemon binary grew past the limit"));
    }
    output.flush()?;
    // Set after the write, so the umask does not decide the mode.
    output.set_permissions(fs::Permissions::from_mode(BINARY_MODE))?;
    output.sync_all()
}
