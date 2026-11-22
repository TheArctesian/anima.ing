//! The checked copy of the history of one repository of the person (ruling of the operator):
//! one bare Git folder for each project in the folder of the records, shared by the threads
//! of that project. The Git folder of the runner for a worktree ([`super::own`]) reads it;
//! no `git` of the runner reads the object store of the person.
//!
//! An agent can write that object store, and can put a link to a different repository in the
//! place of any part of it. A `git` of the runner that reads it, with the rights of the
//! person, would then read a repository that the agent cannot read. So the store is read
//! with the rights of the thread: the runner gives a way to start a program as the agent of
//! a thread is started ([`AsThread`]), and that program makes one pack file of what is
//! missing. The runner takes the pack as a file, `git index-pack` names each object by the
//! hash of its content, and a commit counts as known only when each object of it is there.
//! Only the runner writes this folder, and nothing comes into it without that check.
//!
//! I/O: `git init`, `git index-pack`, `git rev-list`, one run of the thread, a few files.

use std::ffi::OsStr;
use std::path::Path;
use std::time::Duration;

use super::super::project::refs;
use super::super::run::hooks::{AsThread, ThreadRun};
use super::super::run::{Git, GitError, Run, said};
use super::Worktree;
use super::walk::{self, Dir, Make};

/// The refs of the commits whose whole history is in the copy, one for each commit.
const SEEN: &str = "refs/anima/seen";

/// The most commits that are named as known when the thread makes a pack.
const MOST_KNOWN: usize = 64;

/// The time the copy of a history gets: the first one of a large repository is long.
pub const LONG: Duration = Duration::from_hours(1);

/// The folder that the thread writes its pack to, and the file that says what is wanted, in
/// the entry of the worktree in the Git folder of the person.
const OUT: &str = "anima-pack";
const WANT: &str = "anima-want";

/// The most bytes of the list of the commits where a shallow clone ends.
const MOST_SHALLOW: u64 = 16 * 1024 * 1024;

/// Makes the copy for the repository at `project` at `shared` when it is not there. `long_ids` is for a
/// repository whose IDs have 64 digits.
///
/// # Errors
/// A [`GitError`] when `git init` fails.
pub async fn ensure(
    git: &Git,
    (shared, project): (&Path, &Path),
    long_ids: bool,
) -> Result<(), GitError> {
    if shared.join("HEAD").is_file() {
        return Ok(());
    }
    super::own::init(git, shared, long_ids).await?;
    super::disk::name(shared, project).map_err(GitError::Files)
}

/// The refusal of a commit with an object that did not arrive under its ID.
fn forged() -> GitError {
    GitError::Failed {
        code: None,
        said: "an object of the repository is missing or is not what its ID says".to_owned(),
    }
}

fn failed(said: String) -> GitError {
    GitError::Failed { code: None, said }
}

/// Makes each commit of `commits` known: its whole history is copied from the repository of
/// the person into the copy of the project, read with the rights of the thread of `worktree`
/// and checked by the runner. `git` is the one of [`Git::for_worktree`]. A commit that is
/// known already costs one look.
///
/// # Errors
/// A [`GitError`] when the runner has no way to read as the thread, the thread could not
/// make the pack, a commit is no commit, or an object of it is missing or forged.
pub async fn learn(git: &Git, worktree: &Worktree, commits: &[&str]) -> Result<(), GitError> {
    let record = worktree.record();
    let store = &git.in_own(&record.shared).await?;
    let mut wanted = Vec::new();
    for commit in commits {
        let commit = refs::id(commit).ok_or_else(forged)?;
        let seen = format!("{SEEN}/{commit}");
        let known = ["show-ref", "--verify", "--quiet", seen.as_str()];
        if !store.yes(Run::new(&record.shared, known)).await? {
            wanted.push(commit);
        }
    }
    if wanted.is_empty() {
        return Ok(());
    }
    shallow(worktree)?;
    let listed = ["for-each-ref", "--format=%(objectname)", SEEN];
    let listed = store.text(Run::new(&record.shared, listed)).await?;
    let known: Vec<&str> = listed.lines().take(MOST_KNOWN).collect();
    // First without what is known already; when the repository of the person lost one of
    // those commits, the whole history again.
    let mut packed = pack(git, worktree, (&wanted, &known)).await;
    if packed.is_err() && !known.is_empty() {
        packed = pack(git, worktree, (&wanted, &[])).await;
    }
    packed?;
    taken(store, worktree).await?;
    for commit in wanted {
        // Each object of the commit is in the folders of the runner now, or it is refused.
        let itself = format!("{commit}^{{commit}}");
        let is = [
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &itself,
        ];
        let is = git.text(Run::new(worktree.folder(), is)).await;
        let mut whole = vec!["rev-list", "--objects", "--quiet", commit, "--not"];
        whole.extend(&known);
        let whole = Run::new(worktree.folder(), whole).with_time(LONG);
        if is.ok().as_deref() != Some(commit) || git.output(whole).await.is_err() {
            return Err(forged());
        }
        let seen = format!("{SEEN}/{commit}");
        let keep = ["update-ref", "--no-deref", seen.as_str(), commit];
        store.output(Run::new(&record.shared, keep)).await?;
    }
    Ok(())
}

/// Copies the list of the commits where a shallow clone ends into the Git folder of the
/// runner, so that `git` there does not look for their parents. Only lines that are IDs.
fn shallow(worktree: &Worktree) -> Result<(), GitError> {
    let record = worktree.record();
    let common = Dir::open(&record.common_dir).map_err(GitError::Files)?;
    let listed = common.read(walk::name("shallow"), MOST_SHALLOW);
    let listed = listed.map_err(GitError::Files)?.unwrap_or_default();
    let listed = String::from_utf8_lossy(&listed);
    let ends: String = listed
        .lines()
        .filter_map(refs::id)
        .flat_map(|id| [id, "\n"])
        .collect();
    let file = record.own.join("shallow");
    if ends.is_empty() {
        let _ = std::fs::remove_file(file);
        return Ok(());
    }
    std::fs::write(file, ends).map_err(GitError::Files)
}

/// Has the thread make one pack of the history of `wanted` without the history of `known`,
/// in the entry of its worktree in the Git folder of the person.
async fn pack(
    git: &Git,
    worktree: &Worktree,
    (wanted, known): (&[&str], &[&str]),
) -> Result<(), GitError> {
    let as_thread = git.as_thread().ok_or_else(|| {
        failed(
            "the runner gave no way to read the repository with the rights of the thread"
                .to_owned(),
        )
    })?;
    let record = worktree.record();
    let entry = Dir::open(&record.git_dir).map_err(GitError::Files)?;
    let mut list = String::new();
    for commit in wanted {
        list.extend([commit, "\n"]);
    }
    for commit in known {
        list.extend(["^", commit, "\n"]);
    }
    let made = (|| {
        use std::io::Write as _;
        entry.remove_all(walk::name(OUT))?;
        entry.new_folder(walk::name(OUT))?;
        entry.remove(walk::name(WANT), false)?;
        entry
            .new_file(walk::name(WANT), false)?
            .write_all(list.as_bytes())
    })();
    made.map_err(GitError::Files)?;
    let base = record.git_dir.join(OUT).join("p");
    let args: [&OsStr; 4] = [
        "pack-objects".as_ref(),
        "--revs".as_ref(),
        "--quiet".as_ref(),
        base.as_os_str(),
    ];
    let run = ThreadRun {
        thread: worktree.thread(),
        dir: worktree.folder().to_owned(),
        program: git.program().to_owned(),
        args: args.map(OsStr::to_owned).into(),
        env: Vec::new(),
        input: Some(record.git_dir.join(WANT)),
        within: LONG,
    };
    match AsThread::start(as_thread, run).await {
        Ok(ended) if ended.code == Some(0) => Ok(()),
        Ok(ended) => Err(failed(format!(
            "the repository could not be read: {}",
            said::without_credentials(&ended.said)
        ))),
        Err(error) => Err(failed(format!("the repository could not be read: {error}"))),
    }
}

/// Takes each pack that the thread made, as a file with no link followed, and brings it into
/// the copy of the project: `git index-pack` names each object by its content first.
async fn taken(store: &Git, worktree: &Worktree) -> Result<(), GitError> {
    let record = worktree.record();
    let entry = Dir::open(&record.git_dir).map_err(GitError::Files)?;
    let incoming = format!("incoming-{}", worktree.thread().as_simple());
    let incoming = super::own::fresh(&record.shared, &incoming)?;
    let copied = (|| {
        let Some(out) = entry.below(Path::new(OUT), Make::No)? else {
            return Ok(Vec::new());
        };
        let mut packs = Vec::new();
        for name in out.names()? {
            let is_pack = Path::new(&name)
                .extension()
                .is_some_and(|kind| kind == "pack");
            let Some(mut found) = out.file(&name)?.filter(|_| is_pack) else {
                continue;
            };
            let copy = incoming.join(&name);
            std::io::copy(&mut found, &mut std::fs::File::create(&copy)?)?;
            packs.push(copy);
        }
        entry.remove_all(walk::name(OUT))?;
        entry.remove(walk::name(WANT), false)?;
        Ok::<_, std::io::Error>(packs)
    })();
    for pack in copied.map_err(GitError::Files)? {
        let name: [&OsStr; 2] = ["index-pack".as_ref(), pack.as_os_str()];
        store
            .output(Run::new(&record.shared, name).with_time(LONG))
            .await?;
        for file in [pack.clone(), pack.with_extension("idx")] {
            let placed = file
                .file_name()
                .map(|name| record.shared.join("objects/pack").join(name))
                .ok_or(GitError::NotFound)?;
            std::fs::rename(file, placed).map_err(GitError::Files)?;
        }
    }
    let _ = std::fs::remove_dir_all(&incoming);
    Ok(())
}
