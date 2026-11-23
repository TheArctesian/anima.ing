//! The operator writes the sources in `model-sources.json` at the machine; the hub cannot write
//! them. The version lists change only with a signed release (Section 6, CAG-008, SEC-C-005).

pub mod fetch;

use serde::Deserialize;

use super::cause::Cause;
use super::names::{Backend, Runtime, Word};

/// The number of `llama_server` is a placeholder until HT-120 picks the first release (OD-C1).
pub const LOWEST: [(Runtime, u64); 1] = [(Runtime::LlamaServer, 6000)];

pub const ADVISORIES: [(Runtime, u64); 0] = [];

pub const MAX_ARCHIVE_BYTES: u64 = 8 << 30;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Release {
    pub runtime: Runtime,
    pub version: String,
    pub backend: Backend,
}

impl Release {
    /// The other runtimes have no version pattern yet (MW-P4), so no version of them reads.
    #[must_use]
    pub fn number(runtime: Runtime, version: &str) -> Option<u64> {
        let digits = match runtime {
            Runtime::LlamaServer => version.strip_prefix('b')?,
            Runtime::MlxLm | Runtime::Vllm => return None,
        };
        let plain = (1..=9).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit());
        plain.then(|| digits.parse().ok()).flatten()
    }

    #[must_use]
    pub fn folder(&self) -> String {
        format!("{}-{}", self.version, self.backend.as_str())
    }
}

/// # Errors
/// The cause of the first list that refuses the release.
pub fn accepted(release: &Release) -> Result<(), Cause> {
    let number = Release::number(release.runtime, &release.version).ok_or(Cause::MemberUnknown)?;
    let lowest = LOWEST
        .iter()
        .find(|(runtime, _)| *runtime == release.runtime);
    if lowest.is_none_or(|(_, lowest)| number < *lowest) {
        return Err(Cause::RuntimeTooOld);
    }
    if ADVISORIES.contains(&(release.runtime, number)) {
        return Err(Cause::RuntimeAdvisory);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveFormat {
    TarGz,
    Zip,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceEntry {
    pub runtime: String,
    pub version: String,
    pub os: String,
    pub architecture: String,
    pub backend: String,
    /// The HTTPS address of the archive (a `.tar.gz`, `.tgz` or `.zip`).
    pub address: String,
    pub sha256: String,
    pub size_bytes: u64,
    /// The path of the server program inside the archive, as `build/bin/llama-server`.
    pub program: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sources {
    pub sources: Vec<SourceEntry>,
}

impl SourceEntry {
    #[must_use]
    pub fn format(&self) -> Option<ArchiveFormat> {
        format_of(&self.address)
    }
}

#[must_use]
pub fn format_of(address: &str) -> Option<ArchiveFormat> {
    let path = std::path::Path::new(address.split(['?', '#']).next().unwrap_or_default());
    let is = |found: Option<&std::ffi::OsStr>, wanted: &str| {
        found.is_some_and(|found| found.eq_ignore_ascii_case(wanted))
    };
    let inner = path.file_stem().map(std::path::Path::new);
    if is(path.extension(), "tgz")
        || (is(path.extension(), "gz") && is(inner.and_then(std::path::Path::extension), "tar"))
    {
        Some(ArchiveFormat::TarGz)
    } else if is(path.extension(), "zip") {
        Some(ArchiveFormat::Zip)
    } else {
        None
    }
}

fn entry_holds(entry: &SourceEntry) -> bool {
    entry.format().is_some()
        && entry.address.starts_with("https://")
        && super::names::is_lower_hex(&entry.sha256, 64)
        && (1..=MAX_ARCHIVE_BYTES).contains(&entry.size_bytes)
        && super::files::is_file_path(&entry.program)
}

impl Sources {
    /// # Errors
    /// `runtime_not_configured` when no entry names it, or the entry is not whole.
    pub fn find(
        &self,
        release: &Release,
        os: &str,
        architecture: &str,
    ) -> Result<&SourceEntry, Cause> {
        self.sources
            .iter()
            .find(|entry| {
                entry.runtime == release.runtime.as_str()
                    && entry.version == release.version
                    && entry.backend == release.backend.as_str()
                    && entry.os == os
                    && entry.architecture == architecture
            })
            .filter(|entry| entry_holds(entry))
            .ok_or(Cause::RuntimeNotConfigured)
    }
}
