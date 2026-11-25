//! Verification of saved Git LFS payloads, without invoking Git LFS or its extensions.

use std::io::{self, Read as _};
use std::path::Path;

use agent_threads::git::GitRefusal;
use sha2::{Digest as _, Sha256};

use super::{Dir, Git, incomplete, probe};
use crate::threads::git::place::walk::Make;

const POINTER_WINDOW: usize = 1024;

pub(super) async fn verify(
    git: &Git,
    view: &Path,
    common: &Dir,
    oid: &str,
) -> Result<(), GitRefusal> {
    let bytes = git
        .output(probe(view, ["cat-file", "blob", oid]))
        .await
        .map_err(|_| incomplete())?;
    if let Some((oid, size)) = pointer(&bytes[..bytes.len().min(POINTER_WINDOW)])? {
        payload(common, oid, size).map_err(|_| incomplete())?;
    }
    Ok(())
}

fn pointer(bytes: &[u8]) -> Result<Option<(&str, u64)>, GitRefusal> {
    // The LFS smudge reader parses the first 1024 bytes after trimming outer whitespace.
    let text = std::str::from_utf8(bytes).map_err(|_| incomplete())?;
    let mut lines = text.trim().lines().filter(|line| !line.is_empty());
    if !lines.clone().any(version) {
        return Ok(None);
    }
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
        .filter(|hash| hash.len() == 64 && super::super::refs::id(hash).is_some())
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

fn payload(common: &Dir, oid: &str, size: u64) -> io::Result<()> {
    let path = format!("lfs/objects/{}/{}/{oid}", &oid[..2], &oid[2..4]);
    let (leaf, name) = common
        .above(Path::new(&path), Make::Existing)?
        .ok_or(io::ErrorKind::NotFound)?;
    let file = leaf.file(name)?.ok_or(io::ErrorKind::NotFound)?;
    if file.metadata()?.len() != size || !matches_payload(&file, oid, size)? {
        return Err(io::ErrorKind::InvalidData.into());
    }
    file.sync_all()?;
    leaf.sync()
}

fn matches_payload(file: &std::fs::File, oid: &str, size: u64) -> io::Result<bool> {
    let mut reader = file.take(size.saturating_add(1));
    let mut digest = Sha256::new();
    let mut block = [0_u8; 8192];
    loop {
        let count = reader.read(&mut block)?;
        if count == 0 {
            break;
        }
        digest.update(&block[..count]);
    }
    Ok(crate::model::fetch::hex(&digest.finalize()) == oid)
}
