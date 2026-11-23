use std::{fs, path::Path, time::Duration};

use super::error::ProcessLogError;
use crate::os_program::{self, OsProgramError};

/// An absolute path on macOS, so `PATH` cannot swap it.
#[cfg(target_os = "macos")]
pub(super) const OPENER: &str = "/usr/bin/open";
#[cfg(target_os = "linux")]
pub(super) const OPENER: &str = "xdg-open";

/// No shell. The opener's output is discarded: it can echo the path.
pub(super) fn open_with(
    folder: &Path,
    opener: &str,
    patience: Duration,
) -> Result<(), ProcessLogError> {
    fs::create_dir_all(folder).map_err(|_| ProcessLogError::Failed)?;
    os_program::open_within(opener, folder.as_os_str(), patience).map_err(|error| match error {
        OsProgramError::NotFound => ProcessLogError::Unavailable,
        _ => ProcessLogError::Failed,
    })
}

#[cfg(windows)]
pub(super) const OPENER: &str = "explorer.exe";
