//! Checks a content-addressed Git LFS payload in the project's own store.

use std::io::{self, Read as _};
use std::path::PathBuf;

use sha2::{Digest as _, Sha256};

use super::walk::{Dir, Make};

pub(in crate::threads::git) fn is_oid(oid: &str) -> bool {
    oid.len() == 64 && super::super::project::refs::id(oid).is_some()
}

pub(in crate::threads::git) fn path(oid: &str) -> io::Result<PathBuf> {
    if !is_oid(oid) {
        return Err(io::Error::other("the LFS cache has an invalid object name"));
    }
    Ok(PathBuf::from(format!(
        "lfs/objects/{}/{}/{oid}",
        &oid[..2],
        &oid[2..4]
    )))
}

pub(in crate::threads::git) fn verify(common: &Dir, oid: &str, size: u64) -> io::Result<()> {
    let path = path(oid)?;
    let (leaf, name) = common
        .above(&path, Make::Existing)?
        .ok_or(io::ErrorKind::NotFound)?;
    let file = leaf.file(name)?.ok_or(io::ErrorKind::NotFound)?;
    if file.metadata()?.len() != size || !matches_payload(&file, oid, size)? {
        return Err(io::Error::other(
            "a project LFS payload does not match its object name and size",
        ));
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
