//! Verifies an interrupted W1 save independently of the Git store about to be archived.

use std::ffi::OsStr;
use std::io;
use std::path::Path;

use agent_threads::git::GitRefusal;

use super::super::super::place::{Home, Record, records::Directory, shared, walk::Dir};
use super::super::super::run::{Git, Run};

mod blobs;
mod objects;
mod pointer;

const SCRATCH_PREFIX: &str = "verify-retirement-";
const SCRATCH_SUFFIX: usize = 6;

pub(in crate::threads::git::worktree) fn clean_scratch(home: &Home, own: &Path) -> io::Result<()> {
    let private = match Directory::open(home.records(), own, false) {
        Ok(private) => private,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in private.names()? {
        if scratch_name(&entry) {
            private.remove_tree(&entry)?;
        }
    }
    private.sync()
}

fn scratch_name(name: &OsStr) -> bool {
    name.to_str()
        .and_then(|name| name.strip_prefix(SCRATCH_PREFIX))
        .is_some_and(|suffix| {
            suffix.len() == SCRATCH_SUFFIX
                && suffix.bytes().all(|byte| byte.is_ascii_alphanumeric())
        })
}

pub(super) async fn verify(git: &Git, record: &Record, commit: &str) -> Result<(), GitRefusal> {
    let common = Dir::open(&record.common_dir).map_err(|_| incomplete())?;
    let scratch = tempfile::Builder::new()
        .prefix(SCRATCH_PREFIX)
        .rand_bytes(SCRATCH_SUFFIX)
        .tempdir_in(&record.own)
        .map_err(|_| incomplete())?;
    let view = scratch.path();
    let git = git.unbound();
    let format = if commit.len() == 64 {
        "--object-format=sha256"
    } else {
        "--object-format=sha1"
    };
    git.output(probe(
        view,
        ["init", "--bare", "--quiet", "--template=", format],
    ))
    .await
    .map_err(|_| incomplete())?;
    objects::mirror(&common, view).map_err(|error| object_failure(&error))?;
    let kind = probe(view, ["cat-file", "-t", commit]);
    if git.text(kind).await.ok().as_deref() != Some("commit") {
        return Err(incomplete());
    }
    // Full fsck inflates and checks the objects themselves, including symlink blobs.
    let check = [
        "fsck",
        "--full",
        "--no-reflogs",
        "--no-dangling",
        "--no-progress",
        commit,
    ];
    git.output(probe(view, check))
        .await
        .map_err(|_| incomplete())?;
    for blob in blobs::regular_blobs(&git, view, commit).await? {
        pointer::verify(&git, view, &common, &blob).await?;
    }
    Ok(())
}

fn probe(view: &Path, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Run<'_> {
    let lead = std::iter::once(OsStr::new("--no-replace-objects").to_owned());
    let args = lead.chain(args.into_iter().map(|arg| arg.as_ref().to_owned()));
    Run::new(view, args)
        .with_env("GIT_DIR", view)
        .with_env("GIT_CONFIG_NOSYSTEM", "1")
        .with_env("GIT_CONFIG_GLOBAL", "/dev/null")
        .with_env("GIT_ATTR_NOSYSTEM", "1")
        .with_setting("core.attributesFile", "/dev/null")
        .with_time(shared::LONG)
}

const RECOVERY: &str = "Keep the worktree and Anima records. Restore missing or damaged Git objects and LFS payloads to the project before retrying. If verification still fails, recover the saved ref manually before removing the registration.";

pub(super) fn object_failure(error: &io::Error) -> GitRefusal {
    GitRefusal::Failed {
        said: format!("Could not verify the project object files: {error}. {RECOVERY}"),
    }
}

fn incomplete() -> GitRefusal {
    GitRefusal::Failed {
        said: format!("Could not verify the earlier removal snapshot. {RECOVERY}"),
    }
}
