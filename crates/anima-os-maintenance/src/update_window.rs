//! Shared update approval and its boot-scoped hold on new agent and build work.
use crate::{Error, read_receipt, recovery::boot_id};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, io, os::unix::fs::MetadataExt as _, path::Path, time::SystemTime};
use uuid::Uuid;

pub const RECEIPT: &str = "/persist/anima-updates/activation-window.json";
pub const UPDATE_STATE: &str = "/persist/anima-updates/state.json";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DrainScope {
    pub boot: Uuid,
    pub system: String,
    pub previous: String,
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Scheduled,
    Deferred,
    Expired,
    Changed,
    Cancelled,
    Dispatching,
    NeedsReview,
}

impl Phase {
    #[must_use]
    pub fn pending(self) -> bool {
        matches!(self, Self::Scheduled | Self::Deferred)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Window {
    pub id: Uuid,
    pub version: String,
    pub review: String,
    pub starts_at: u64,
    pub ends_at: u64,
    pub phase: Phase,
    pub reason: Option<String>,
    // An earlier restart-only approval has no authority to pause admissions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drain: Option<DrainScope>,
}

impl Window {
    /// Read the shared approval from the updater's private state directory.
    /// # Errors
    /// The directory or approved window is unreadable or untrusted.
    pub fn read() -> Result<Option<Self>, Error> {
        let parent = Path::new(RECEIPT).parent().ok_or(Error::Refused(
            "The activation window directory is invalid.",
        ))?;
        match fs::symlink_metadata(parent) {
            Ok(metadata)
                if metadata.is_dir() && metadata.uid() == 0 && metadata.mode() & 0o022 == 0 => {}
            Ok(_) => {
                return Err(Error::Refused(
                    "The activation window directory is not protected.",
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        }
        read_receipt(RECEIPT, 4096)
    }
}

/// # Errors
/// The system clock is earlier than the Unix epoch.
pub fn now() -> Result<u64, Error> {
    unix_seconds(SystemTime::now())
}

fn unix_seconds(time: SystemTime) -> Result<u64, Error> {
    Ok(time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|_| Error::Refused("The system clock is unavailable."))?
        .as_secs())
}

// The window changes between admission transactions. A launch already under
// the admission lock can finish its dispatch checks at the window boundary.
pub(super) fn active_until(observed_at: SystemTime) -> Result<Option<u64>, Error> {
    let Some(window) = Window::read()?.filter(|window| window.phase.pending()) else {
        return Ok(None);
    };
    let Some(scope) = window.drain else {
        return Ok(None);
    };
    if !(window.starts_at..window.ends_at).contains(&unix_seconds(observed_at)?)
        || scope.boot != boot_id()?
    {
        return Ok(None);
    }
    let selected = read_receipt::<Value>(UPDATE_STATE, 65_536)?.is_some_and(|staged| {
        staged["phase"] == "staged"
            && staged["version"] == window.version
            && staged["system"] == scope.system
            && staged["previous"] == scope.previous
    });
    Ok(selected.then_some(window.ends_at))
}
