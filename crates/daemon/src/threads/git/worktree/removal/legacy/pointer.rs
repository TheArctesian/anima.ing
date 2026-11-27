//! Verification of saved Git LFS payloads, without invoking Git LFS or its extensions.

use std::path::Path;

use agent_threads::git::GitRefusal;

use super::{Dir, Git, incomplete, probe};
use crate::threads::git::place::lfs;

const POINTER_WINDOW: u64 = 1024;

pub(super) async fn verify(
    git: &Git,
    view: &Path,
    common: &Dir,
    oid: &str,
) -> Result<(), GitRefusal> {
    let bytes = git
        .output(probe(view, ["cat-file", "blob", oid]).with_output_prefix(POINTER_WINDOW))
        .await
        .map_err(|_| incomplete())?;
    if let Some((oid, size)) = pointer(&bytes)?
        && size != 0
    {
        lfs::verify(common, oid, size).map_err(|_| GitRefusal::Failed {
            said: format!(
                "Could not verify LFS payload {oid} ({size} bytes) at lfs/objects/{}/{}/{oid} inside the project Git directory. {} Ordinary pointer-shaped text also needs manual recovery when legacy filter provenance is unavailable.",
                &oid[..2], &oid[2..4], super::RECOVERY
            ),
        })?;
    }
    Ok(())
}

fn pointer(bytes: &[u8]) -> Result<Option<(&str, u64)>, GitRefusal> {
    // Only candidate headers need strict decoding. Ordinary filtered blobs can be binary.
    let header = String::from_utf8_lossy(bytes);
    let first = lines(&header).find(|line| !line.starts_with("ext-"));
    if !first.is_some_and(version) {
        return Ok(None);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| incomplete())?;
    let mut lines = lines(text);
    let head = lines.next().ok_or_else(incomplete)?;
    let hash = lines
        .next()
        .and_then(|line| line.strip_prefix("oid sha256:"));
    let size = lines.next().and_then(|line| line.strip_prefix("size "));
    // An extension (including one before the version) requires its own recovery program.
    if !version(head) || lines.next().is_some() {
        return Err(incomplete());
    }
    let hash = hash
        .filter(|hash| lfs::is_oid(hash))
        .ok_or_else(incomplete)?;
    let size = size
        .and_then(|text| text.parse::<i64>().ok())
        .and_then(|size| u64::try_from(size).ok())
        .ok_or_else(incomplete)?;
    Ok(Some((hash, size)))
}

fn version(line: &str) -> bool {
    matches!(
        line,
        "version https://git-lfs.github.com/spec/v1"
            | "version https://hawser.github.com/spec/v1"
            | "version http://git-media.io/v/2"
    )
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.trim().lines().filter(|line| !line.is_empty())
}
