//! The one shape of each read of a thread. The launcher of the runner starts a read for each
//! thread, also a confined one, only when [`is_read`] says yes: so a read is never more than
//! what this engine sends.
//!
//! Pure: it only looks at the run.

use std::ffi::{OsStr, OsString};
use std::path::{Component, Path};

use super::super::super::super::run::hooks::{Kind, ThreadRun};
use super::{CLOSED, PREFIX, QUIET};

/// The start of each read of a thread, after [`CLOSED`]: it lists the objects of commits.
pub(super) const LIST: [&str; 4] = ["rev-list", "--objects", "--quiet", "--end-of-options"];

/// The start of each read of a thread that makes a pack, after [`CLOSED`].
pub(super) const PACK: [&str; 3] = ["pack-objects", "--revs", "--quiet"];

/// Whether `run` is a read of a thread as this engine makes it, and nothing else: the kind
/// [`Kind::Read`], `program` as given, a folder that is a full path, the closed options first,
/// and one of the two reads with its environment:
///
/// - `rev-list` of commits, then the known commits with `^`; the environment of [`QUIET`];
///   no input.
/// - `pack-objects` into the folder of the pack of the thread in its worktree; the
///   environment of [`QUIET`], the object folder in that folder, and the store of the person
///   as one more store; the input is the list `want` of the runner's folder of a worktree of
///   this thread in `records`.
///
/// The launcher opens the input with no link followed.
#[must_use]
pub fn is_read(run: &ThreadRun, program: &Path, records: &Path) -> bool {
    let Some(rest) = run.args.strip_prefix(CLOSED.map(OsString::from).as_slice()) else {
        return false;
    };
    let plain = run.kind == Kind::Read
        && run.program == program
        && run.dir.is_absolute()
        && no_dots(&run.dir);
    let (quiet, more) = (QUIET.len(), run.env.len());
    let quiet_first = more >= quiet
        && run.env[..quiet]
            .iter()
            .zip(QUIET)
            .all(|((name, value), (set_name, set))| *name == set_name && value == set);
    plain
        && quiet_first
        && match (&run.input, &run.env[quiet.min(more)..]) {
            (None, []) => lists(rest),
            (Some(input), [objects, store]) => {
                packs(rest, run, (objects, store)) && is_want(input, run, records)
            }
            _ => false,
        }
}

/// `rev-list` of one or more commits, then the known commits, each with `^`.
fn lists(args: &[OsString]) -> bool {
    let Some(rest) = args.strip_prefix(LIST.map(OsString::from).as_slice()) else {
        return false;
    };
    let wanted = rest
        .iter()
        .take_while(|arg| is_id(arg.as_encoded_bytes()))
        .count();
    let known = &rest[wanted..];
    wanted > 0
        && known
            .iter()
            .all(|arg| arg.as_encoded_bytes().strip_prefix(b"^").is_some_and(is_id))
}

/// `pack-objects` into `p` of the folder of the pack of the thread, with the object folder in
/// it and the store of the person as one more store.
fn packs(
    args: &[OsString],
    run: &ThreadRun,
    (objects, store): (&(&'static str, OsString), &(&'static str, OsString)),
) -> bool {
    let Some([base]) = args.strip_prefix(PACK.map(OsString::from).as_slice()) else {
        return false;
    };
    let ours = run.dir.join(format!("{PREFIX}{}", run.thread.as_simple()));
    let store_path = Path::new(&store.1);
    *base == ours.join("p").into_os_string()
        && objects.0 == "GIT_OBJECT_DIRECTORY"
        && objects.1 == ours.join("objects").into_os_string()
        && store.0 == "GIT_ALTERNATE_OBJECT_DIRECTORIES"
        && store_path.is_absolute()
        && no_dots(store_path)
        && !no_list(&store.1)
}

/// The input is `<records>/<this thread>/<a name>.git/want`.
fn is_want(input: &Path, run: &ThreadRun, records: &Path) -> bool {
    let Ok(below) = input.strip_prefix(records.join(run.thread.to_string())) else {
        return false;
    };
    let parts: Vec<_> = below.components().collect();
    matches!(
        parts.as_slice(),
        [Component::Normal(own), Component::Normal(want)]
            if *want == "want" && Path::new(own).extension() == Some(OsStr::new("git"))
    )
}

/// `path` has no `.` or `..` part.
fn no_dots(path: &Path) -> bool {
    path.components()
        .all(|part| matches!(part, Component::RootDir | Component::Normal(_)))
}

/// `path` cannot be one entry of a list of folders of `git`: it has the separator of the
/// list, a line end, or starts with a quote.
#[must_use]
pub(super) fn no_list(path: &OsStr) -> bool {
    let bytes = path.as_encoded_bytes();
    bytes
        .iter()
        .any(|byte| matches!(byte, b':' | b';' | b'\n' | b'\r'))
        || bytes.first() == Some(&b'"')
}

/// An object ID of `git`: 40 or 64 small hex digits.
fn is_id(id: &[u8]) -> bool {
    matches!(id.len(), 40 | 64)
        && id
            .iter()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}
