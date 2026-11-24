//! The version of the programs in a copy; an update never puts an older set over a newer one.

use std::fs;
use std::path::Path;

use super::{CopyError, io_error};
use crate::Folders;

/// This program's version: `ANIMA_VERSION` from the release build, else the package version.
pub const THIS: &str = match option_env!("ANIMA_VERSION") {
    Some(version) => version,
    None => env!("CARGO_PKG_VERSION"),
};

const FILE: &str = "version";

fn numbers(text: &str) -> Option<[u64; 3]> {
    let mut parts = text.trim().split('.').map(|part| part.parse::<u64>().ok());
    let found = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(found)
}

/// Whether `carried` is lower than `installed`; a missing or malformed version blocks nothing.
#[must_use]
pub fn is_older(carried: &str, installed: Option<&str>) -> bool {
    match (numbers(carried), installed.and_then(numbers)) {
        (Some(carried), Some(installed)) => carried < installed,
        _ => false,
    }
}

/// The version of the copy in `programs/current`, when it has a version file.
#[must_use]
pub fn installed(folders: &Folders) -> Option<String> {
    let text = crate::plain_file::read_text(&folders.programs().join(FILE), 64).ok()?;
    Some(text.trim().to_owned())
}

pub(crate) fn write(next: &Path, version: &str) -> Result<(), CopyError> {
    let path = next.join(FILE);
    fs::write(&path, format!("{version}\n")).map_err(io_error("write", &path))
}
