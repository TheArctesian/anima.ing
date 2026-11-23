//! Files only the running user can read: created `0600` from the start in the folder they will
//! live in, then renamed into place (MET-013, K8 Section 2).

#[cfg(unix)]
use std::fs::File;
use std::io::{self, Read as _, Write as _};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
use std::path::Path;

use zeroize::Zeroizing;

const MODE: u32 = 0o600;

/// The most a private file may hold: a token record or the configuration is a few hundred bytes.
pub const LIMIT: usize = 64 * 1024;

#[must_use]
pub const fn exposed(mode: u32, owner: u32, running: u32) -> bool {
    mode & 0o777 != MODE || owner != running
}

#[must_use]
pub const fn folder_exposed(mode: u32, owner: u32, running: u32) -> bool {
    mode & 0o077 != 0 || owner != running
}

/// Others can write, and no sticky bit keeps them to their own entries (as in `/tmp`).
#[must_use]
pub const fn parent_replaceable(mode: u32) -> bool {
    mode & 0o002 != 0 && mode & 0o1000 == 0
}

/// The folders above are read with links unresolved; their owner and group access are not checked.
///
/// # Errors
/// An I/O error when the folder or one above it cannot be read.
#[cfg(unix)]
pub fn folder_private(dir: &Path) -> io::Result<bool> {
    let metadata = std::fs::symlink_metadata(dir)?;
    let running = running_user()?;
    if !metadata.is_dir() || folder_exposed(metadata.permissions().mode(), metadata.uid(), running)
    {
        return Ok(false);
    }
    for parent in dir.ancestors().skip(1) {
        if !parent.as_os_str().is_empty()
            && parent_replaceable(std::fs::metadata(parent)?.permissions().mode())
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// # Errors
/// The path or its security descriptor cannot be inspected.
#[cfg(windows)]
pub fn folder_private(dir: &Path) -> io::Result<bool> {
    if !std::fs::symlink_metadata(dir)?.is_dir() || !anima_windows::files::secure(dir, true)? {
        return Ok(false);
    }
    for parent in dir.ancestors().skip(1) {
        if !anima_windows::files::secure(parent, false)? {
            return Ok(false);
        }
    }
    Ok(true)
}

#[derive(Debug, thiserror::Error)]
pub enum PrivateFileError {
    #[error("file access failed: {0}")]
    Io(#[from] io::Error),
    #[error("the file is not mode 0600 and owned by the user running the daemon")]
    Exposed,
}

/// # Errors
/// [`PrivateFileError::Io`] when any step fails; the old file is then left as it was.
pub fn write(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), PrivateFileError> {
    // `tempfile` creates the file with mode 0600, exclusively, so no other process can have it.
    let mut temporary = tempfile::Builder::new().prefix(".tmp-").tempfile_in(dir)?;
    #[cfg(windows)]
    anima_windows::files::protect(temporary.path())?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist(dir.join(name))
        .map_err(|error| error.error)?;
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    Ok(())
}

/// `None` when `path` does not exist.
///
/// # Errors
/// [`PrivateFileError::Exposed`] unless `0600` and the running user's; [`PrivateFileError::Io`].
pub fn read(path: &Path) -> Result<Option<Zeroizing<Vec<u8>>>, PrivateFileError> {
    read_within(path, LIMIT)
}

/// # Errors
/// As [`read`]; [`PrivateFileError::Io`] also for a file of more than `limit` bytes.
pub fn read_within(
    path: &Path,
    limit: usize,
) -> Result<Option<Zeroizing<Vec<u8>>>, PrivateFileError> {
    #[cfg(unix)]
    let opened = File::open(path);
    #[cfg(windows)]
    let opened = anima_windows::files::read(path, true);
    let file = match opened {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    #[cfg(unix)]
    let metadata = file.metadata()?;
    #[cfg(unix)]
    if exposed(
        metadata.permissions().mode(),
        metadata.uid(),
        running_user()?,
    ) {
        return Err(PrivateFileError::Exposed);
    }
    Ok(Some(read_limited(file, limit)?))
}

/// One buffer with room for `limit + 1` from the start never moves, so its wipe covers every byte
/// read (LOG-007).
///
/// # Errors
/// [`io::ErrorKind::FileTooLarge`] when `reader` holds more than `limit` bytes; the read error.
pub fn read_limited(reader: impl io::Read, limit: usize) -> io::Result<Zeroizing<Vec<u8>>> {
    let mut bytes = Zeroizing::new(Vec::with_capacity(limit + 1));
    reader.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            format!("more than {limit} bytes"),
        ));
    }
    Ok(bytes)
}

/// The owner of an unnamed temporary file, which works on Linux and macOS alike.
///
/// # Errors
/// An I/O error when the temporary file cannot be made.
#[cfg(unix)]
pub fn running_user() -> io::Result<u32> {
    Ok(tempfile::tempfile()?.metadata()?.uid())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_private_records_round_trip_with_acl_checks_and_size_bounds() {
        let temporary = tempfile::tempdir().unwrap();
        let home = crate::config::home::Home::at(temporary.path().join("daemon"));
        home.ensure().unwrap();
        write(home.dir(), "record", b"first").unwrap();
        let path = home.dir().join("record");
        assert!(anima_windows::files::secure(&path, true).unwrap());
        assert_eq!(read(&path).unwrap().unwrap().as_slice(), b"first");
        write(home.dir(), "record", b"replacement").unwrap();
        assert_eq!(read(&path).unwrap().unwrap().as_slice(), b"replacement");
        assert!(read_within(&path, 4).is_err());
        assert!(read(&home.dir().join("missing")).unwrap().is_none());
    }

    #[test]
    fn met_019_a_private_file_is_read_into_one_buffer_that_never_moves() {
        let bytes = read_limited(&b"0123456789"[..], 16).expect("read");
        assert_eq!(&bytes[..], b"0123456789");
        assert_eq!(
            bytes.capacity(),
            17,
            "room made at the start; the buffer never grew"
        );
        let full = read_limited(&[7_u8; 16][..], 16).expect("exactly the limit");
        assert_eq!((full.len(), full.capacity()), (16, 17));
        let over = read_limited(&[7_u8; 17][..], 16).expect_err("one byte too many");
        assert_eq!(over.kind(), io::ErrorKind::FileTooLarge);
    }

    #[test]
    fn met_013_a_file_is_exposed_unless_it_is_0600_and_the_runner_s() {
        assert!(!exposed(0o100_600, 1000, 1000));
        assert!(exposed(0o100_600, 1001, 1000), "another owner");
        assert!(exposed(0o100_644, 1000, 1000), "readable by others");
        assert!(exposed(0o100_400, 1000, 1000), "not exactly 0600");
        assert!(exposed(0o100_660, 1000, 1000));
    }

    #[test]
    fn met_013_a_folder_is_exposed_when_group_or_others_have_any_access_or_it_is_not_the_runner_s()
    {
        assert!(!folder_exposed(0o040_700, 1000, 1000));
        assert!(!folder_exposed(0o040_500, 1000, 1000));
        assert!(folder_exposed(0o040_750, 1000, 1000));
        assert!(folder_exposed(0o040_701, 1000, 1000));
        assert!(folder_exposed(0o040_700, 0, 1000));
    }

    #[test]
    fn met_013_a_parent_others_can_write_is_replaceable_unless_it_is_sticky() {
        assert!(parent_replaceable(0o040_777));
        assert!(parent_replaceable(0o040_702));
        assert!(!parent_replaceable(0o041_777), "sticky, like /tmp");
        assert!(
            !parent_replaceable(0o040_775),
            "group may write, others may not"
        );
        assert!(!parent_replaceable(0o040_755));
    }
}
