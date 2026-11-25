//! Enumerates legacy regular blobs without assuming missing filter provenance.

use std::path::Path;

use agent_threads::git::GitRefusal;

use super::{Git, incomplete, probe};

type Blob<'a> = (&'a [u8], &'a str);

pub(super) async fn regular_blobs(
    git: &Git,
    view: &Path,
    commit: &str,
) -> Result<Vec<String>, GitRefusal> {
    let listing = git
        .output(probe(view, ["ls-tree", "-r", "-z", "--full-tree", commit]))
        .await
        .map_err(|_| incomplete())?;
    // W1 did not retain filter provenance, including external attribute macros.
    // Every regular blob must therefore be checked for a pointer candidate.
    Ok(blobs(&listing)?
        .into_iter()
        .map(|(_, oid)| oid.to_owned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect())
}

fn blobs(listing: &[u8]) -> Result<Vec<Blob<'_>>, GitRefusal> {
    fields(listing)?
        .map(|entry| {
            let at = entry
                .iter()
                .position(|byte| *byte == b'\t')
                .ok_or_else(incomplete)?;
            let header = std::str::from_utf8(&entry[..at]).map_err(|_| incomplete())?;
            let path = &entry[at + 1..];
            let (mode, object) = header.split_once(' ').ok_or_else(incomplete)?;
            if !mode.starts_with("100") {
                // Symlinks and Git links are not filtered during checkout.
                return Ok(None);
            }
            let oid = object
                .strip_prefix("blob ")
                .and_then(super::super::refs::id)
                .ok_or_else(incomplete)?;
            Ok(Some((path, oid)))
        })
        .filter_map(Result::transpose)
        .collect()
}

fn fields(bytes: &[u8]) -> Result<impl Iterator<Item = &[u8]>, GitRefusal> {
    if !bytes.is_empty() && !bytes.ends_with(&[0]) {
        return Err(incomplete());
    }
    Ok(bytes
        .split_inclusive(|byte| *byte == 0)
        .map(|field| &field[..field.len() - 1]))
}
