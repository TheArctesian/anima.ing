//! Recognizes LFS object stores without reading personal Git configuration.

use std::ffi::OsStr;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use super::{Dir, Make, is_oid, path};
use crate::threads::git::run::{Git, GitError, Run};

const DEFAULT_OBJECTS: &str = "lfs/objects";
const MAX_DEPTH: u8 = 128;

/// Check only the runner-created config. Storage supplied by an external config
/// is detected from the private cache inventory below.
pub(in crate::threads::git) async fn check_storage(git: &Git, own: &Path) -> Result<(), GitError> {
    let dir = Dir::open(own).map_err(GitError::Files)?;
    let config = dir
        .read(OsStr::new("config"), 64 * 1024)
        .map_err(GitError::Files)?
        .ok_or_else(|| GitError::Files(unresolved()))?;
    let args = [
        "config",
        "--file",
        "-",
        "--no-includes",
        "--null",
        "--get-all",
        "lfs.storage",
    ];
    let lookup = Run::new(own, args)
        .with_env("GIT_DIR", own)
        .with_env("GIT_CONFIG_NOSYSTEM", "1")
        .with_env("GIT_CONFIG_GLOBAL", "/dev/null")
        .with_input(config);
    let (status, values, warnings) = git.unbound().finished(lookup).await?;
    let default = match status {
        Some(1) => values.is_empty(),
        Some(0) => values.strip_suffix(&[0]).is_some_and(|values| {
            values
                .split(|byte| *byte == 0)
                .all(|value| matches!(value, b"lfs" | b"./lfs"))
        }),
        _ => false,
    };
    if default && warnings.is_empty() {
        Ok(())
    } else {
        Err(GitError::Files(unresolved()))
    }
}

pub(in crate::threads::git) fn visit(
    own: &Path,
    mut payload: impl FnMut(File, &str) -> io::Result<()>,
) -> io::Result<()> {
    let own = Dir::open(own)?;
    let mut folders = vec![(PathBuf::new(), 0)];
    while let Some((relative, depth)) = folders.pop() {
        let dir = own
            .below(&relative, Make::No)?
            .ok_or(io::ErrorKind::NotFound)?;
        for name in dir.names()? {
            let at = relative.join(&name);
            if dir.link(&name).is_some() {
                return Err(unresolved());
            }
            if dir.below(Path::new(&name), Make::No)?.is_some() {
                if depth == MAX_DEPTH {
                    return Err(unresolved());
                }
                folders.push((at, depth + 1));
            } else if let Some(oid) = cached_oid(&at, &name)? {
                let file = dir.file(&name)?.ok_or(io::ErrorKind::InvalidData)?;
                payload(file, oid)?;
            }
        }
    }
    Ok(())
}

fn cached_oid<'a>(at: &Path, name: &'a OsStr) -> io::Result<Option<&'a str>> {
    let oid = name.to_str().filter(|oid| is_oid(oid));
    if at.starts_with(DEFAULT_OBJECTS) {
        let oid = oid.ok_or_else(unresolved)?;
        if path(oid)? != at {
            return Err(unresolved());
        }
        return Ok(Some(oid));
    }
    // A custom relative lfs.storage can put this object layout anywhere under the runner Git dir.
    if let Some(oid) = oid
        && at.ends_with(path(oid)?.strip_prefix("lfs").map_err(io::Error::other)?)
    {
        return Err(unresolved());
    }
    Ok(None)
}

fn unresolved() -> io::Error {
    io::Error::other(
        "a custom, linked or unrecognized LFS cache cannot be matched safely to the project. Keep the worktree and Anima records and recover its LFS payloads manually",
    )
}
