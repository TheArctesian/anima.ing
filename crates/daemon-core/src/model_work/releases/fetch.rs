//! The rules of the child fetch process (Section 4, R-336), shared by the child and the daemon.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::{ArchiveFormat, MAX_ARCHIVE_BYTES, format_of};
use crate::model_work::cause::Cause;
use crate::model_work::names::{Runtime, Word as _, is_lower_hex};

pub const MAX_REDIRECTS: usize = 5;

pub const MAX_ORDER_BYTES: u64 = 64 << 10;

pub const MAX_REPLY_BYTES: u64 = 64;

pub const DONE: &str = "done";

pub const CHILD_CAUSES: [Cause; 6] = [
    Cause::Checksum,
    Cause::Size,
    Cause::SourceFailed,
    Cause::LimitDisk,
    Cause::ArchiveRejected,
    Cause::RuntimeNotConfigured,
];

/// The one variable that macOS gives each process, even with an empty environment; it holds no
/// secret and no address.
pub const SYSTEM_VARIABLE: &str = "__CF_USER_TEXT_ENCODING";

pub fn environment_empty<N: AsRef<std::ffi::OsStr>>(mut names: impl Iterator<Item = N>) -> bool {
    names.all(|name| name.as_ref() == SYSTEM_VARIABLE)
}

/// Only a release of the daemon changes these hosts.
#[must_use]
pub const fn release_hosts(runtime: Runtime) -> &'static [&'static str] {
    match runtime {
        Runtime::LlamaServer => &[
            "github.com",
            "objects.githubusercontent.com",
            "release-assets.githubusercontent.com",
        ],
        Runtime::MlxLm | Runtime::Vllm => &[],
    }
}

#[must_use]
pub fn redirect_permitted(scheme: &str, host: Option<&str>, hosts: &[&str]) -> bool {
    let named = |host: &str| {
        host.parse::<std::net::IpAddr>().is_err() && !host.starts_with('[') && hosts.contains(&host)
    };
    scheme == "https" && host.is_some_and(named)
}

#[must_use]
pub fn first_host_permitted(address: &str, hosts: &[&str]) -> bool {
    let Some(rest) = address.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.contains(['@', ':']) && redirect_permitted("https", Some(authority), hosts)
}

#[must_use]
pub fn follows(previous: usize, scheme: &str, host: Option<&str>, hosts: &[&str]) -> bool {
    previous < MAX_REDIRECTS && redirect_permitted(scheme, host, hosts)
}

#[must_use]
pub fn download_deadline(size_bytes: u64) -> Duration {
    Duration::from_secs((300 + size_bytes / (1 << 20)).min(3_600))
}

/// Past it, the child gives back `archive_rejected`.
pub const UNPACK_DEADLINE: Duration = Duration::from_mins(25);

/// Five minutes past the child's own deadlines, so the child gives its own cause first.
#[must_use]
pub fn child_deadline(size_bytes: u64) -> Duration {
    download_deadline(size_bytes) + UNPACK_DEADLINE + Duration::from_mins(5)
}

/// It holds no token, no hub origin and no proxy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FetchOrder {
    pub runtime: String,
    pub address: String,
    pub sha256: String,
    pub size_bytes: u64,
    /// The folder where the child makes the unnamed file of the archive.
    pub archive_folder: String,
    /// The new empty folder, made by the daemon, that the child unpacks into.
    pub unpack_folder: String,
    /// The free disk that the unpack leaves.
    pub reserve: u64,
    /// The process id of the daemon. On Linux the child ends when its parent is no longer this
    /// process.
    pub parent: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checked {
    pub runtime: Runtime,
    pub format: ArchiveFormat,
    pub hosts: &'static [&'static str],
}

impl FetchOrder {
    /// # Errors
    /// `runtime_not_configured` for each part that does not hold.
    pub fn checked(&self) -> Result<Checked, Cause> {
        let runtime = Runtime::parse(&self.runtime).ok_or(Cause::RuntimeNotConfigured)?;
        let format = format_of(&self.address).ok_or(Cause::RuntimeNotConfigured)?;
        let absolute = |folder: &str| std::path::Path::new(folder).is_absolute();
        let hosts = release_hosts(runtime);
        let holds = first_host_permitted(&self.address, hosts)
            && is_lower_hex(&self.sha256, 64)
            && (1..=MAX_ARCHIVE_BYTES).contains(&self.size_bytes)
            && absolute(&self.archive_folder)
            && absolute(&self.unpack_folder);
        holds
            .then_some(Checked {
                runtime,
                format,
                hosts,
            })
            .ok_or(Cause::RuntimeNotConfigured)
    }
}

#[must_use]
pub fn reply(outcome: Result<(), Cause>) -> &'static str {
    match outcome {
        Ok(()) => DONE,
        Err(cause) if CHILD_CAUSES.contains(&cause) => cause.as_str(),
        Err(_) => Cause::FetchFailed.as_str(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ending<'a> {
    /// The child exited with code 0 and wrote `reply`.
    Exited(&'a str),
    /// The child ended with another code, by a signal, or could not be read.
    Fault,
    /// The deadline passed; the daemon ended the child.
    Stalled,
}

/// A fault, a stall and a reply that is not a word of [`CHILD_CAUSES`] are `fetch_failed`
/// (R-406); no text of the child goes into the cause.
///
/// # Errors
/// The cause of the child, or `fetch_failed`.
pub fn verdict(ending: Ending<'_>) -> Result<(), Cause> {
    let Ending::Exited(text) = ending else {
        return Err(Cause::FetchFailed);
    };
    let word = text.strip_suffix('\n').unwrap_or(text);
    if word == DONE {
        return Ok(());
    }
    Err(CHILD_CAUSES
        .into_iter()
        .find(|cause| cause.as_str() == word)
        .unwrap_or(Cause::FetchFailed))
}
