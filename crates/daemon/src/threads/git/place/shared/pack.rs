//! The runs of a thread for the checked copy of a project, and the packs that they leave: the
//! only reads of the object store of the person (THR-050).
//!
//! The program is the `git` of this computer, started by the runner as the agent of the thread
//! is started ([`AsThread`]). It reads the configuration of the repository, which each thread
//! of the project can write, also a confined one while this thread has more rights. So the
//! run gets the same closed set as a run of the runner: no fetch of a missing object, no
//! transport, no `ssh` command, no monitor program, no hook. With them `git pack-objects` and
//! `git rev-list` start no program, whatever the configuration says.
//!
//! I/O: one run of the thread; files of the worktree taken with no link followed.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use super::super::super::run::hooks::{AsThread, Kind, ThreadRun};
use super::super::super::run::{Git, GitError, Run, said};
use super::super::Worktree;
use super::super::walk::{Dir, Make};
use super::{LONG, failed};

/// The most packs that one run of a thread can leave.
const MOST_PACKS: usize = 16;

/// What each run of a thread gets before its own arguments, where it wins over each file of
/// configuration. Only options that the oldest `git` of a supported system knows: `git` 2.43
/// (Ubuntu 24.04) refuses `--no-lazy-fetch`, so the fetch of a missing object is closed by the
/// environment ([`QUIET`]) and, for a `git` that does not read it, by `protocol.allow`.
const CLOSED: [&str; 21] = [
    "--no-optional-locks",
    "--no-pager",
    "-c",
    "protocol.allow=never",
    "-c",
    "core.sshCommand=false",
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "core.alternateRefsCommand=",
    "--no-replace-objects",
    // How a pack is made is not for the repository to say: a pack that is not packed is
    // many times larger.
    "-c",
    "core.compression=-1",
    "-c",
    "pack.compression=-1",
    "-c",
    "pack.window=10",
    "-c",
    "pack.depth=50",
];

/// The environment of each run of a thread: no fetch of a missing object, no prompt.
/// `GIT_NO_LAZY_FETCH` is what `--no-lazy-fetch` sets in `git` 2.44 and later, and it also
/// reaches each `git` that this one starts.
const QUIET: [(&str, &str); 3] = [
    ("GIT_NO_LAZY_FETCH", "1"),
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_SSH_COMMAND", "false"),
];

/// The name of the folder in the worktree that the thread writes its pack to: the one place
/// that each thread can write, also a confined one whose sandbox keeps the Git folder
/// read-only. It is there only while a copy runs, and its name has the ID of the thread.
#[must_use]
pub fn folder(worktree: &Worktree) -> OsString {
    format!("{PREFIX}{}", worktree.thread().as_simple()).into()
}

/// The start of the name of each such folder.
pub const PREFIX: &str = ".anima-pack-";

mod shape;

pub use shape::is_read;
use shape::{LIST, PACK};

/// Runs `git <args>` with the rights of the thread of `worktree`, in its worktree, with the
/// file `input` as standard input. `false` when `git` ended with a code that is not 0.
///
/// # Errors
/// A [`GitError`] when the runner gave no way to run as the thread, or the run did not start.
async fn as_thread(
    git: &Git,
    worktree: &Worktree,
    (args, input): (Vec<OsString>, Option<&Path>),
    objects: Vec<(&'static str, OsString)>,
) -> Result<Result<(), String>, GitError> {
    let start = git.as_thread().ok_or_else(|| {
        failed(
            "the runner gave no way to read the repository with the rights of the thread"
                .to_owned(),
        )
    })?;
    let closed = CLOSED.iter().map(OsString::from);
    let run = ThreadRun {
        kind: Kind::Read,
        thread: worktree.thread(),
        dir: worktree.folder().to_owned(),
        program: git.program().to_owned(),
        args: closed.chain(args).collect(),
        env: QUIET
            .iter()
            .map(|(name, value)| (*name, OsString::from(value)))
            .chain(objects)
            .collect(),
        input: input.map(Path::to_owned),
        within: LONG,
    };
    match AsThread::start(start, run).await {
        Ok(ended) if ended.code == Some(0) => Ok(Ok(())),
        Ok(ended) => Ok(Err(said::without_credentials(&ended.said))),
        Err(error) => Err(failed(format!("the repository could not be read: {error}"))),
    }
}

/// Whether the thread can read each object of the history of `commits` that is not in the
/// history of `known`, in the repository of the person. No content is read: `git` looks only
/// whether each object is there for it.
///
/// # Errors
/// As [`as_thread`].
pub async fn reads(
    git: &Git,
    worktree: &Worktree,
    (commits, known): (&[&str], &[&str]),
) -> Result<bool, GitError> {
    let args = list_args(commits, known);
    Ok(as_thread(git, worktree, (args, None), Vec::new())
        .await?
        .is_ok())
}

/// The arguments of the read of [`reads`], after the closed options: `git` refuses `--not`
/// after the commits, so each known commit has `^` instead.
#[must_use]
pub fn list_args(commits: &[&str], known: &[&str]) -> Vec<OsString> {
    let mut args: Vec<OsString> = LIST.map(Into::into).into();
    args.extend(commits.iter().map(OsString::from));
    args.extend(
        known
            .iter()
            .map(|known| OsString::from(format!("^{known}"))),
    );
    args
}

/// The options that each read of a thread gets first.
#[must_use]
pub fn closed() -> Vec<OsString> {
    CLOSED.map(OsString::from).into()
}

/// Has the thread make one pack of the history of `wanted` without the history of `known`,
/// in a folder of its worktree.
///
/// # Errors
/// A [`GitError`] when the thread could not make the pack.
pub async fn make(
    git: &Git,
    worktree: &Worktree,
    (wanted, known): (&[&str], &[&str]),
) -> Result<(), GitError> {
    let record = worktree.record();
    let mut list = String::new();
    for commit in wanted {
        list.extend([commit, "\n"]);
    }
    for commit in known {
        list.extend(["^", commit, "\n"]);
    }
    // The list is a file of the runner: the runner opens it for the program.
    let want = record.own.join("want");
    std::fs::write(&want, list).map_err(GitError::Files)?;
    // The folder is made new by the runner, with no link followed on the way.
    let name = folder(worktree);
    let top = Dir::open(worktree.folder()).map_err(GitError::Files)?;
    let made = top.remove_all(&name).and_then(|()| top.new_folder(&name));
    made.map_err(GitError::Files)?;
    let store = Path::new(&name).join("objects");
    let below = |path: &Path| {
        top.below(path, Make::No)?
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
    };
    below(Path::new(&name))
        .and_then(|ours| ours.new_folder("objects".as_ref()))
        .and_then(|()| below(&store))
        .and_then(|objects| objects.new_folder("pack".as_ref()))
        .map_err(GitError::Files)?;
    let at = worktree.folder().join(&name);
    let args = PACK.map(OsString::from);
    let args = args
        .into_iter()
        .chain([at.join("p").into_os_string()])
        .collect();
    // `git` writes the files of a pack in a folder of the object store first. The store of
    // the person stays as it is: the thread writes only in the folder of its pack, and reads
    // the store of the person as one more store.
    let store = record.common_dir.join("objects");
    if shape::no_list(store.as_os_str()) {
        return Err(failed(
            "the path of the repository has a character that a list of folders of git cannot hold"
                .to_owned(),
        ));
    }
    let objects = vec![
        ("GIT_OBJECT_DIRECTORY", at.join("objects").into_os_string()),
        ("GIT_ALTERNATE_OBJECT_DIRECTORIES", store.into_os_string()),
    ];
    let packed = as_thread(git, worktree, (args, Some(&want)), objects).await;
    let packed = packed.and_then(|ended| {
        ended.map_err(|said| failed(format!("the repository could not be read: {said}")))
    });
    if packed.is_err() {
        // The folder of the pack does not stay in the worktree of the thread.
        let _ = top.remove_all(&name);
    }
    packed
}

/// Takes each pack that the thread made, as a file with no link followed, into the quarantine
/// at `quarantine` (the `git` of `checked` runs with it). `git index-pack` names each object
/// by its content, and each pack gets the name that `git` gives it from its bytes. At most
/// `left` bytes are taken: what a thread leaves is not copied without end.
///
/// # Errors
/// A [`GitError`] when a pack cannot be copied, is no pack, or the packs are larger than
/// `left`.
pub async fn take(
    checked: &Git,
    (worktree, quarantine): (&Worktree, &Path),
    left: u64,
) -> Result<(), GitError> {
    let name = folder(worktree);
    let incoming = super::super::own::fresh(quarantine, "incoming")?;
    let copied = (|| {
        use std::io::Read as _;
        let top = Dir::open(worktree.folder())?;
        let mut packs = Vec::new();
        let mut left = left;
        if let Some(out) = top.below(Path::new(&name), Make::No)? {
            for name in out.names()?.into_iter().take(MOST_PACKS) {
                let is_pack = Path::new(&name).extension() == Some(OsStr::new("pack"));
                let Some(found) = out.file(&name)?.filter(|_| is_pack) else {
                    continue;
                };
                // The name in the folder of the runner is one that the runner chose.
                let copy = incoming.join(format!("in-{}.pack", packs.len()));
                // One byte more than what is left shows a pack that is too large.
                let mut found = found.take(left.saturating_add(1));
                let taken = std::io::copy(&mut found, &mut std::fs::File::create(&copy)?)?;
                left = left.checked_sub(taken).ok_or_else(|| {
                    std::io::Error::other(
                        "the thread left more than the copy of this project can hold",
                    )
                })?;
                packs.push(copy);
            }
        }
        Ok::<_, std::io::Error>(packs)
    })();
    // The folder of the thread goes on each way out.
    if let Ok(top) = Dir::open(worktree.folder()) {
        let _ = top.remove_all(&name);
    }
    for pack in copied.map_err(GitError::Files)? {
        let index: [&OsStr; 2] = ["index-pack".as_ref(), pack.as_os_str()];
        let index = Run::new(quarantine, index).with_time(LONG);
        let named = checked.text(index).await?;
        // `git` prints the name of the pack: the hash of what it holds.
        let named = named.split_whitespace().next_back().unwrap_or_default();
        let named = super::super::super::project::refs::id(named)
            .ok_or_else(|| failed("git gave no name for a pack of the repository".to_owned()))?;
        for kind in ["pack", "idx"] {
            let placed = quarantine.join(format!("objects/pack/pack-{named}.{kind}"));
            std::fs::rename(pack.with_extension(kind), placed).map_err(GitError::Files)?;
        }
    }
    Ok(())
}
