//! The copy of before after a good update (R-437): it stays beside the new copy as
//! `anima-daemon.<its version>`, so the person can go back by hand. Only the last one stays.

use std::{
    fs::{self, File},
    io,
    path::Path,
};

use super::super::{DaemonError, version::Version};

/// Renames the copy of before at `aside` to `<copy>.<version>` and deletes the copies kept by
/// earlier updates. A copy of before whose version is not known is deleted.
///
/// # Errors
/// [`DaemonError::Remove`] when the copy of before could not be renamed or deleted.
pub(super) fn keep(copy: &Path, aside: &Path, before: Option<Version>) -> Result<(), DaemonError> {
    let (Some(folder), Some(name)) = (copy.parent(), copy.file_name()) else {
        return Err(DaemonError::Remove);
    };
    let Some([major, minor, patch]) = before else {
        return match fs::remove_file(aside) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(DaemonError::Remove),
        };
    };
    let base = name.to_string_lossy();
    let kept = format!("{base}.{major}.{minor}.{patch}");
    fs::rename(aside, folder.join(&kept)).map_err(|_| DaemonError::Remove)?;
    for entry in fs::read_dir(folder).into_iter().flatten().flatten() {
        let other = entry.file_name();
        let other = other.to_string_lossy();
        let earlier = other != kept
            && other
                .strip_prefix(&*base)
                .and_then(|rest| rest.strip_prefix('.'))
                .is_some_and(is_version);
        // Only a plain file is deleted; a link or a folder with such a name stays.
        if earlier && entry.file_type().is_ok_and(|kind| kind.is_file()) {
            let _ = fs::remove_file(entry.path());
        }
    }
    File::open(folder)
        .and_then(|opened| opened.sync_all())
        .map_err(|_| DaemonError::Remove)
}

/// Whether `text` is three numbers with dots between them.
fn is_version(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}
