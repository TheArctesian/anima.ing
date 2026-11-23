//! After a good update the copy of before stays as `anima-daemon.<version>`, so the person can
//! go back by hand; only the last one stays (R-437).

use std::{
    fs::{self, File},
    io,
    path::Path,
};

use super::super::{DaemonError, version::Version};

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
        if earlier && entry.file_type().is_ok_and(|kind| kind.is_file()) {
            let _ = fs::remove_file(entry.path());
        }
    }
    File::open(folder)
        .and_then(|opened| opened.sync_all())
        .map_err(|_| DaemonError::Remove)
}

fn is_version(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}
