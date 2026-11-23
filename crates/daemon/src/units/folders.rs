//! The I/O around [`super::trust`]: the checks of the binary, its folders and the unit's folder (MET-019).

use std::fs;
use std::io;
use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _};
use std::path::{Path, PathBuf};

use super::trust::{self, Installer, Stat};

#[derive(Debug, thiserror::Error)]
pub enum FolderError {
    #[error(
        "{} is not owned by root or this user, or users other than its owner may write it (through its group, an ACL or as others), so the daemon could be swapped: run `chmod go-w` on it (and `setfacl -b` for an ACL) or reinstall it",
        .0.display()
    )]
    UntrustedBinary(PathBuf),
    #[error(
        "{} is not owned by root or this user, or other users may write to it (through its group, an ACL or as others), so the daemon or its unit could be swapped. A folder its group may write, such as Debian's /usr/local/bin (root:staff 2775) or Homebrew's on macOS (you:admin 0775), is fixed with `chmod g-w` on it; otherwise move the binary",
        .0.display()
    )]
    UntrustedFolder(PathBuf),
    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> FolderError + '_ {
    move |source| FolderError::Io {
        path: path.to_owned(),
        source,
    }
}

fn stat(path: &Path) -> Result<Stat, FolderError> {
    let metadata = fs::metadata(path).map_err(io_error(path))?;
    Ok(Stat {
        mode: metadata.mode(),
        owner: metadata.uid(),
        group: metadata.gid(),
        acl: has_acl(path)?,
    })
}

#[cfg(target_os = "linux")]
fn has_acl(path: &Path) -> Result<bool, FolderError> {
    use rustix::io::Errno;
    match rustix::fs::getxattr_query(path, "system.posix_acl_access") {
        Ok(_) => Ok(true),
        Err(Errno::NODATA | Errno::NOTSUP) => Ok(false),
        Err(error) => Err(io_error(path)(error.into())),
    }
}

/// macOS ACLs (`chmod +a`) do not show in the mode and are not read here.
#[cfg(not(target_os = "linux"))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the same signature as the Linux reader"
)]
fn has_acl(_path: &Path) -> Result<bool, FolderError> {
    Ok(false)
}

/// The binary, by its real path, and every folder above it.
pub(super) fn check_binary(program: &Path, by: Installer) -> Result<(), FolderError> {
    if !trust::binary_trusted(stat(program)?, by) {
        return Err(FolderError::UntrustedBinary(program.to_owned()));
    }
    check_folders(program.parent().unwrap_or_else(|| Path::new("/")), by)
}

/// `folder` (a real path) and every folder above it.
fn check_folders(folder: &Path, by: Installer) -> Result<(), FolderError> {
    for folder in folder.ancestors() {
        if !trust::folder_trusted(stat(folder)?, by) {
            return Err(FolderError::UntrustedFolder(folder.to_owned()));
        }
    }
    Ok(())
}

/// Each missing part is made in a checked folder, then resolved and checked, so nothing is made through a link to a folder others can write.
pub(super) fn make_folder(dir: &Path, by: Installer) -> Result<PathBuf, FolderError> {
    let existing = dir
        .ancestors()
        .find(|folder| fs::metadata(folder).is_ok())
        .unwrap_or_else(|| Path::new("/"));
    let mut real = existing.canonicalize().map_err(io_error(existing))?;
    check_folders(&real, by)?;
    let missing = dir.strip_prefix(existing).unwrap_or_else(|_| Path::new(""));
    for part in missing.components() {
        real.push(part);
        // 0755: a folder the unit goes in is public, and only its owner may change it.
        match fs::DirBuilder::new().mode(0o755).create(&real) {
            Err(error) if error.kind() != io::ErrorKind::AlreadyExists => {
                return Err(io_error(&real)(error));
            }
            _ => {}
        }
        real = real.canonicalize().map_err(io_error(&real))?;
        check_folders(&real, by)?;
    }
    Ok(real)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn met_019_the_untrusted_folder_message_names_the_group_writable_cases_and_the_fix() {
        let text = FolderError::UntrustedFolder(PathBuf::from("/usr/local/bin")).to_string();
        assert!(
            text.contains("Debian's /usr/local/bin (root:staff 2775)"),
            "{text}"
        );
        assert!(
            text.contains("Homebrew's on macOS (you:admin 0775)"),
            "{text}"
        );
        assert!(text.contains("`chmod g-w`"), "{text}");
        assert!(text.contains("an ACL"), "{text}");
    }

    #[test]
    fn met_019_the_untrusted_binary_message_names_an_acl_too() {
        let text = FolderError::UntrustedBinary(PathBuf::from("/opt/anima-daemon")).to_string();
        assert!(text.contains("an ACL"), "{text}");
        assert!(text.contains("`setfacl -b`"), "{text}");
    }
}
