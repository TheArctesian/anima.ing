//! Verifies the saved commit's checkout in the original object store before using its ref.

use std::ffi::{OsStr, OsString};
use std::io::{self, Read as _};
use std::path::Path;

use agent_threads::git::GitRefusal;
use sha2::{Digest as _, Sha256};

use super::super::super::place::{
    Record, shared,
    walk::{Dir, Make},
};
use super::super::super::run::{Git, Run};

const VERSION: &str = "^version (https://git-lfs.github.com/spec/v1|https://hawser.github.com/spec/v1|http://git-media.io/v/2)";
const POINTER_LIMIT: usize = 1024;

pub(super) async fn verify(git: &Git, record: &Record, commit: &str) -> Result<(), GitRefusal> {
    let common = Dir::open(&record.common_dir).map_err(|_| incomplete())?;
    if common
        .below(Path::new("objects"), Make::No)
        .map_err(|_| incomplete())?
        .is_none()
    {
        return Err(incomplete());
    }
    // Configuration comes from the private runner repository, while objects must be
    // readable from the original repository. There is no fetch, filter or replacement ref.
    let git = git.unbound();
    let kind = probe(record, ["cat-file", "-t", commit]);
    if git.text(kind).await.ok().as_deref() != Some("commit") {
        return Err(incomplete());
    }
    let tree = format!("{commit}^{{tree}}");
    let reachable = [
        "rev-list",
        "--objects",
        "--quiet",
        "--missing=error",
        &tree,
        "--",
    ];
    git.output(probe(record, reachable))
        .await
        .map_err(|_| incomplete())?;
    let grep = [
        "grep",
        "-l",
        "-z",
        "-E",
        "--no-textconv",
        "-e",
        VERSION,
        commit,
        "--",
    ];
    let (status, matched, warnings) = git
        .finished(probe(record, grep))
        .await
        .map_err(|_| incomplete())?;
    if !matches!(status, Some(0 | 1)) || !warnings.is_empty() {
        return Err(incomplete());
    }
    for name in matched
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
    {
        verify_pointer(&git, record, &common, name).await?;
    }
    Ok(())
}

fn probe(record: &Record, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Run<'_> {
    let lead = std::iter::once(OsString::from("--no-replace-objects"));
    let args = lead.chain(args.into_iter().map(|arg| arg.as_ref().to_owned()));
    Run::new(&record.own, args)
        .with_env("GIT_DIR", &record.own)
        .with_env("GIT_OBJECT_DIRECTORY", record.common_dir.join("objects"))
        .with_env("GIT_CONFIG_NOSYSTEM", "1")
        .with_env("GIT_CONFIG_GLOBAL", "/dev/null")
        .with_time(shared::LONG)
}

async fn verify_pointer(
    git: &Git,
    record: &Record,
    common: &Dir,
    name: &[u8],
) -> Result<(), GitRefusal> {
    #[cfg(unix)]
    let name = {
        use std::os::unix::ffi::OsStringExt as _;
        OsString::from_vec(name.to_vec())
    };
    #[cfg(not(unix))]
    let name = OsStr::new(std::str::from_utf8(name).map_err(|_| incomplete())?);
    let size = git
        .text(probe(
            record,
            [OsStr::new("cat-file"), OsStr::new("-s"), &name],
        ))
        .await
        .map_err(|_| incomplete())?
        .parse::<usize>()
        .map_err(|_| incomplete())?;
    if size >= POINTER_LIMIT {
        return Ok(());
    }
    let bytes = git
        .output(probe(
            record,
            [OsStr::new("cat-file"), OsStr::new("blob"), &name],
        ))
        .await
        .map_err(|_| incomplete())?;
    if let Some((oid, size)) = pointer(&bytes) {
        payload(common, oid, size).map_err(|_| incomplete())?;
    }
    Ok(())
}

fn pointer(bytes: &[u8]) -> Option<(&str, u64)> {
    let mut lines = std::str::from_utf8(bytes).ok()?.lines();
    let version = lines.next()?.strip_prefix("version ")?;
    if !matches!(
        version,
        "https://git-lfs.github.com/spec/v1"
            | "https://hawser.github.com/spec/v1"
            | "http://git-media.io/v/2"
    ) {
        return None;
    }
    let oid = lines.find_map(|line| line.strip_prefix("oid sha256:"))?;
    let size = lines
        .find_map(|line| line.strip_prefix("size "))?
        .parse()
        .ok()?;
    (oid.len() == 64 && super::refs::id(oid).is_some()).then_some((oid, size))
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

fn incomplete() -> GitRefusal {
    GitRefusal::Failed {
        said: "The earlier removal snapshot is incomplete or could not be verified. Its records were kept for recovery.".into(),
    }
}
