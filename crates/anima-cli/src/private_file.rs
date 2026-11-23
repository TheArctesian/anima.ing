//! Files only the running user may read: folder `0700`, file `0600`, replaced by rename (CMD-012).

use std::fs;
#[cfg(unix)]
use std::fs::DirBuilder;
#[cfg(unix)]
use std::fs::File;
use std::io::{self, Read as _, Write as _};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _, PermissionsExt as _};
use std::path::Path;

use zeroize::Zeroizing;

/// The most a file of the command line may hold; a config or token file is a few kilobytes.
pub const LIMIT: usize = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum PrivateFileError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[cfg_attr(windows, error("the file permissions allow access by another user"))]
    #[cfg_attr(
        unix,
        error("it is not mode 0600 and owned by you; fix it with chmod 600")
    )]
    Exposed,
    #[error("it is larger than {LIMIT} bytes")]
    TooLarge,
}

/// A file others can read, or that another user owns, is refused: its token could be taken.
///
/// # Errors
/// [`PrivateFileError`] when it is exposed, too large or cannot be read.
pub fn read(path: &Path) -> Result<Option<Zeroizing<Vec<u8>>>, PrivateFileError> {
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
    if metadata.permissions().mode() & 0o077 != 0 || metadata.uid() != running_user()? {
        return Err(PrivateFileError::Exposed);
    }
    let mut bytes = Zeroizing::new(Vec::with_capacity(LIMIT.min(64 * 1024)));
    file.take(LIMIT as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > LIMIT {
        return Err(PrivateFileError::TooLarge);
    }
    Ok(Some(bytes))
}

/// # Errors
/// [`PrivateFileError::Io`] when a step fails; the old file is then left as it was.
pub fn write(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), PrivateFileError> {
    save(dir, name, bytes, Replace::Yes)
}

/// As [`write`], but fails with [`io::ErrorKind::AlreadyExists`] when `dir/name` is there.
///
/// # Errors
/// [`PrivateFileError::Io`] when a step fails or the file exists.
pub fn write_new(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), PrivateFileError> {
    save(dir, name, bytes, Replace::No)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Replace {
    Yes,
    No,
}

fn save(dir: &Path, name: &str, bytes: &[u8], replace: Replace) -> Result<(), PrivateFileError> {
    #[cfg(unix)]
    DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    #[cfg(windows)]
    anima_windows::files::directory(dir)?;
    // `tempfile` creates the file `0600` and exclusively, so no other process can have it.
    let mut temporary = tempfile::Builder::new().prefix(".tmp-").tempfile_in(dir)?;
    #[cfg(windows)]
    anima_windows::files::protect(temporary.path())?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    let destination = dir.join(name);
    match replace {
        Replace::Yes => temporary.persist(destination),
        Replace::No => temporary.persist_noclobber(destination),
    }
    .map_err(|error| error.error)?;
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    Ok(())
}

/// A missing file counts as deleted.
///
/// # Errors
/// [`PrivateFileError::Io`] when it cannot be deleted.
pub fn remove(path: &Path) -> Result<(), PrivateFileError> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

/// The owner of a file this process creates.
#[cfg(unix)]
fn running_user() -> io::Result<u32> {
    Ok(tempfile::tempfile()?.metadata()?.uid())
}
