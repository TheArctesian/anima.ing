//! The rules of a pull (Section 7, PULL-004 to PULL-009), shared by the pull process and the
//! daemon: the hosts, the order, the parts of a resume and the partial copies.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::cause::Cause;
use super::files::{self, MAX_FILES};
use super::names::{Format, Word as _, is_lower_hex, is_source_name};
use super::order::{PullFile, PullSource};
use crate::wire::Progress;

/// Hugging Face and its file hosts; only a release of the daemon changes them (Section 4).
pub const HOSTS: [&str; 5] = [
    "huggingface.co",
    "cdn-lfs.hf.co",
    "cdn-lfs-us-1.hf.co",
    "cas-bridge.xethub.hf.co",
    "us.aws.cdn.hf.co",
];

/// A resume continues from the last complete part (PULL-006).
pub const PART_BYTES: u64 = 16 << 20;

/// A fifth partial copy is refused (`disk_share`).
pub const MAX_PARTIAL: usize = 4;

/// A partial copy that no pull touched for this time is deleted.
pub const PARTIAL_AGE: Duration = Duration::from_hours(7 * 24);

/// The words that the pull process gives back; each other cause is `fetch_failed`.
pub const PULL_CAUSES: [Cause; 5] = [
    Cause::Checksum,
    Cause::Size,
    Cause::SourceFailed,
    Cause::LimitDisk,
    Cause::MemberUnknown,
];

/// What the daemon gives the pull process on its standard input. It holds no token, no hub
/// origin and no proxy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PullOrder {
    pub copy: Uuid,
    pub source: PullSource,
    pub format: String,
    pub files: Vec<PullFile>,
    /// The model cache: the process writes in its `partial/` and moves to its `copies/`.
    pub models: String,
    /// The free disk that the pull leaves (`limit_disk` below it).
    pub reserve: u64,
    /// The process id of the daemon; on Linux the process ends when its parent changes.
    pub parent: u32,
}

impl PullOrder {
    /// Each member again by the rules of Section 2: the process trusts nothing of its input.
    ///
    /// # Errors
    /// `member_unknown` for each part that does not hold.
    pub fn checked(&self) -> Result<(), Cause> {
        let format = Format::parse(&self.format).ok_or(Cause::MemberUnknown)?;
        let source = &self.source;
        let named = is_source_name(&source.organisation)
            && is_source_name(&source.repository)
            && is_lower_hex(&source.revision, 40);
        let listed = (1..=MAX_FILES).contains(&self.files.len())
            && self.files.iter().all(|file| file_holds(file, format))
            && flat_names_unique(&self.files);
        let holds = named && listed && std::path::Path::new(&self.models).is_absolute();
        holds.then_some(()).ok_or(Cause::MemberUnknown)
    }

    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        total_bytes(&self.files)
    }
}

fn file_holds(file: &PullFile, format: Format) -> bool {
    files::is_file_path(&file.name)
        && files::permitted(format, &file.name)
        && file.size_bytes > 0
        && is_lower_hex(&file.sha256, 64)
}

fn flat_names_unique(list: &[PullFile]) -> bool {
    let mut names: Vec<String> = list
        .iter()
        .map(|file| files::flat_name(&file.name).to_ascii_lowercase())
        .collect();
    names.sort_unstable();
    names.dedup();
    names.len() == list.len()
}

#[must_use]
pub fn total_bytes(list: &[PullFile]) -> u64 {
    list.iter()
        .fold(0, |sum, file| sum.saturating_add(file.size_bytes))
}

/// The parts of the address after the host, each one path segment (Section 7, SF-381):
/// `<organisation>/<repository>/resolve/<revision>/<folder>/<file>`.
#[must_use]
pub fn address_segments<'a>(source: &'a PullSource, name: &'a str) -> Vec<&'a str> {
    let head = [
        source.organisation.as_str(),
        source.repository.as_str(),
        "resolve",
        source.revision.as_str(),
    ];
    head.into_iter().chain(name.split('/')).collect()
}

/// Where the download of a file continues: `None` when its `have` bytes are the whole file; 0
/// when it holds more than its size; else the end of its last complete part (PULL-006).
#[must_use]
pub const fn resume_from(have: u64, size: u64) -> Option<u64> {
    if have == size {
        None
    } else if have > size {
        Some(0)
    } else {
        Some(have - have % PART_BYTES)
    }
}

/// Where the body of an answer starts in the file: `asked` for a 206 whose range starts there, 0
/// for a 200 (the file starts again).
///
/// # Errors
/// `source_failed` for each other status, and for a 206 of a different range.
pub fn range_start(status: u16, asked: u64, content_range: Option<&str>) -> Result<u64, Cause> {
    let starts_at_asked = || {
        content_range
            .and_then(|range| range.strip_prefix("bytes "))
            .and_then(|range| range.split_once('-'))
            .is_some_and(|(start, _)| start.parse::<u64>() == Ok(asked))
    };
    match status {
        200 => Ok(0),
        206 if asked > 0 && starts_at_asked() => Ok(asked),
        _ => Err(Cause::SourceFailed),
    }
}

/// The time that a pull of `size_bytes` may take: 10 minutes and 256 KiB per second, at most a
/// day.
#[must_use]
pub fn pull_deadline(size_bytes: u64) -> Duration {
    Duration::from_secs((600 + size_bytes / (256 << 10)).min(24 * 3_600))
}

/// The deadline of the pull process at the daemon: the download, a read again of each byte at
/// 32 MiB per second, and 5 minutes, so the process gives its own cause first.
#[must_use]
pub fn child_deadline(size_bytes: u64) -> Duration {
    pull_deadline(size_bytes) + Duration::from_secs(300 + size_bytes / (32 << 20))
}

/// Whether a partial copy whose newest change was `age` ago is deleted.
#[must_use]
pub fn stale(age: Duration) -> bool {
    age >= PARTIAL_AGE
}

/// Whether a pull may make its partial copy when `others` partial copies of other copies stay.
#[must_use]
pub const fn partial_room(others: usize, own_exists: bool) -> bool {
    own_exists || others < MAX_PARTIAL
}

/// Whether the bytes still to download fit above the reserve (PULL-008).
#[must_use]
pub const fn fits(free: u64, total: u64, have: u64, reserve: u64) -> bool {
    free >= total.saturating_sub(have).saturating_add(reserve)
}

/// The `progress` of a report (PULL-007): the bytes on disk, never above the total, and the
/// rate since the report before, `seconds` ago.
#[must_use]
pub fn progress(bytes: u64, before: u64, total: u64, seconds: u64) -> Progress {
    let bytes = bytes.min(total);
    Progress {
        bytes,
        total_bytes: total,
        bytes_per_second: bytes.saturating_sub(before) / seconds.max(1),
    }
}
