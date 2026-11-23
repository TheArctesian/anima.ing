//! The cache of `huggingface_hub` (Section 10.2): `models--<organisation>--<repository>/` holds
//! `snapshots/<commit>/<file>`, each a link to `../../blobs/<name>`; the name of a blob of an LFS
//! file is its SHA-256. A blob can itself be a link to the shared store `blobs/<xx>/<name>` of the
//! cache (see [`stored_of_link`]).

use crate::model_work::names::{is_lower_hex, is_source_name};

const PREFIX: &str = "models--";
const LINK_PREFIX: &str = "../../blobs/";

/// The organisation and the repository of a folder of the cache.
#[must_use]
pub fn repository(folder: &str) -> Option<(&str, &str)> {
    let (organisation, repository) = folder.strip_prefix(PREFIX)?.split_once("--")?;
    (is_source_name(organisation) && is_source_name(repository))
        .then_some((organisation, repository))
}

/// A folder of `snapshots/`: a commit, 40 lower-case hexadecimal characters.
#[must_use]
pub fn is_commit(name: &str) -> bool {
    is_lower_hex(name, 40)
}

/// The blob that the text of a link of `snapshots/<commit>/` names; `None` for each other text,
/// so no other link is followed (Section 2.3).
#[must_use]
pub fn blob_of_link(text: &[u8]) -> Option<&str> {
    let name = std::str::from_utf8(text).ok()?.strip_prefix(LINK_PREFIX)?;
    (is_lower_hex(name, 64) || is_lower_hex(name, 40)).then_some(name)
}

/// The file of the shared store that the text of a link of `blobs/` names, as its folder and
/// its name: `../../blobs/<xx>/<name>`, with a name of 64 lower-case hexadecimal characters that
/// starts with `<xx>`. The downloader of Unsloth Studio keeps each large file once there, in the
/// folder `blobs/` of the cache, and links each repository to it. `None` for each other text, so
/// no other link is followed (Section 2.3).
#[must_use]
pub fn stored_of_link(text: &[u8]) -> Option<(&str, &str)> {
    let rest = std::str::from_utf8(text).ok()?.strip_prefix(LINK_PREFIX)?;
    let (shard, name) = rest.split_once('/')?;
    (shard.len() == 2 && is_lower_hex(name, 64) && name.starts_with(shard)).then_some((shard, name))
}

/// The SHA-256 that names a blob, for the blob of an LFS file only.
#[must_use]
pub fn blob_sha256(name: &str) -> Option<&str> {
    is_lower_hex(name, 64).then_some(name)
}

/// The name that the hub shows: `<organisation>/<repository>/<file>`.
#[must_use]
pub fn shown_name(organisation: &str, repository: &str, file: &str) -> String {
    format!("{organisation}/{repository}/{file}")
}
