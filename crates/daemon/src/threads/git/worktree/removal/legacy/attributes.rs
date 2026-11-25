//! Git resolves the saved tree's attributes without running any filters.

use std::path::Path;

use agent_threads::git::GitRefusal;

use super::{Git, incomplete, probe};

type Blob<'a> = (&'a [u8], &'a str);

pub(super) async fn lfs_blobs(
    git: &Git,
    view: &Path,
    commit: &str,
) -> Result<Vec<String>, GitRefusal> {
    let listing = git
        .output(probe(view, ["ls-tree", "-r", "-z", "--full-tree", commit]))
        .await
        .map_err(|_| incomplete())?;
    let blobs = blobs(&listing)?;
    if blobs.is_empty() {
        return Ok(Vec::new());
    }
    git.output(probe(view, ["read-tree", commit]))
        .await
        .map_err(|_| incomplete())?;
    let mut paths = Vec::new();
    for (path, _) in &blobs {
        paths.extend_from_slice(path);
        paths.push(0);
    }
    let query =
        probe(view, ["check-attr", "--cached", "-z", "--stdin", "filter"]).with_input(paths);
    let (code, answer, warnings) = git.finished(query).await.map_err(|_| incomplete())?;
    if code != Some(0) || !warnings.is_empty() {
        return Err(incomplete());
    }
    selected(&answer, blobs)
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

fn selected(answer: &[u8], blobs: Vec<Blob<'_>>) -> Result<Vec<String>, GitRefusal> {
    let mut answer = fields(answer)?;
    let mut checked = std::collections::BTreeSet::new();
    for (path, oid) in blobs {
        if answer.next() != Some(path) || answer.next() != Some(b"filter") {
            return Err(incomplete());
        }
        if answer.next().ok_or_else(incomplete)? == b"lfs" {
            checked.insert(oid.to_owned());
        }
    }
    if answer.next().is_some() {
        return Err(incomplete());
    }
    Ok(checked.into_iter().collect())
}

fn fields(bytes: &[u8]) -> Result<impl Iterator<Item = &[u8]>, GitRefusal> {
    if !bytes.is_empty() && !bytes.ends_with(&[0]) {
        return Err(incomplete());
    }
    Ok(bytes
        .split_inclusive(|byte| *byte == 0)
        .map(|field| &field[..field.len() - 1]))
}
