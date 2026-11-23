//! The hooks of the repository of the person (ruling of the operator): a thread with full
//! access may run them for a commit and a push of the person; a confined thread never does.
//!
//! [`of`] is the one place that decides it. The runner's own `git` cannot run a hook of the
//! repository: it does not read that Git folder ([`super::super::project`]). A hook runs only
//! through [`AsThread`], which the runner gives to the engine and which starts a program with
//! the rights of the thread, not the rights of the runner.
//!
//! A hook is a file that each thread of the repository can write, also a confined one. So the
//! files of the hooks are part of what the person accepts ([`Hooks::entries`]): a hook that
//! changed since then stops the commit until the person accepts it. The file of a hook is
//! started directly, not through `git hook run`, so a hook that somebody writes into the
//! configuration of the repository in the meantime does not run with it.
//!
//! I/O: reads the files of the hooks; writes the files that a hook reads into the Git folder
//! of the runner.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use agent_threads::git::GitRefusal;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::super::place::Worktree;
use super::settings::{self, Setting};
use super::{Git, Run, said};

/// The hooks that a commit and a push of the person run, in this order.
const NAMES: [&str; 4] = ["pre-commit", "commit-msg", "post-commit", "pre-push"];

/// The time one hook gets: a hook of a commit can run the tests of a project.
const HOOK_TIME: std::time::Duration = std::time::Duration::from_secs(600);

/// The most bytes of the file of one hook; a larger file is no hook for Anima.
const MOST: u64 = 1024 * 1024;

/// How much the agent of a thread may do on this computer, as the runner set it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Access {
    /// The agent is held to its worktree: no hook of the repository runs for its thread.
    #[default]
    Confined,
    /// The agent runs with the rights of the person: the hooks of the repository may run.
    Full,
}

/// What a run of a thread is for. The launcher checks each kind in its own way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A read of the history of the repository of the person with the rights of the thread,
    /// for each thread, also a confined one. It has one exact shape, which
    /// [`super::super::place::shared::pack::is_read`] checks.
    Read,
    /// A hook of the repository: only for a thread in Full access ([`of`]).
    Hook,
}

/// One program to start for a thread: `program` with `args` in `dir`, with `env` added and
/// the file `input`, when there is one, as its standard input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadRun {
    pub kind: Kind,
    pub thread: Uuid,
    pub dir: PathBuf,
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(&'static str, OsString)>,
    pub input: Option<PathBuf>,
    /// The run is ended, with each process that it started, when it takes longer.
    pub within: std::time::Duration,
}

/// How a run of a thread ended: its exit code and what it wrote to standard error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ended {
    pub code: Option<i32>,
    pub said: String,
}

type Running = Pin<Box<dyn Future<Output = std::io::Result<Ended>> + Send>>;

/// Starts a program as the agent of a thread is started: with the rights, the environment
/// and the confinement of that thread. The runner gives it; the engine has no other way to
/// run a hook.
#[derive(Clone)]
pub struct AsThread(Arc<dyn Fn(ThreadRun) -> Running + Send + Sync>);

impl AsThread {
    pub fn new(start: impl Fn(ThreadRun) -> Running + Send + Sync + 'static) -> Self {
        Self(Arc::new(start))
    }

    /// Starts `run` and waits for its end.
    ///
    /// # Errors
    /// The error of the runner when the program did not start or was ended.
    pub async fn start(&self, run: ThreadRun) -> std::io::Result<Ended> {
        (self.0)(run).await
    }
}

impl std::fmt::Debug for AsThread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AsThread")
    }
}

/// The hooks of the repository for one action of the person in a thread with full access:
/// each file of a hook that is there, with the digest of its content.
#[derive(Clone, Debug)]
pub struct Hooks<'a> {
    start: &'a AsThread,
    worktree: &'a Worktree,
    found: BTreeMap<&'static str, (PathBuf, String)>,
}

/// The hooks that an action of the person runs in the thread of `worktree`: `None`, and so no
/// hook at all, for a confined thread and for a runner that gave no way to start one.
/// `listed` is the configuration of the repository as text; it says where the hooks are.
#[must_use]
pub fn of<'a>(
    access: Access,
    start: Option<&'a AsThread>,
    (worktree, listed): (&'a Worktree, &[Setting]),
) -> Option<Hooks<'a>> {
    let start = match access {
        Access::Full => start?,
        Access::Confined => return None,
    };
    let folder = match settings::hooks_path(listed) {
        // As git: a path that is not absolute is below the top of the worktree.
        Some(set) => worktree.folder().join(set),
        None => worktree.record().common_dir.join("hooks"),
    };
    let found = NAMES
        .into_iter()
        .filter_map(|name| {
            let file = folder.join(name);
            let digest = Sha256::digest(runnable(&file)?);
            Some((name, (file, crate::model::fetch::hex(&digest))))
        })
        .collect();
    Some(Hooks {
        start,
        worktree,
        found,
    })
}

/// The bytes of the file of a hook; `None` when it is no file that can run.
fn runnable(file: &Path) -> Option<Vec<u8>> {
    let about = std::fs::metadata(file).ok()?;
    #[cfg(unix)]
    let runs = std::os::unix::fs::PermissionsExt::mode(&about.permissions()) & 0o111 != 0;
    #[cfg(not(unix))]
    let runs = true;
    if !about.is_file() || !runs || about.len() > MOST {
        return None;
    }
    std::fs::read(file).ok()
}

/// What one hook gets besides its arguments.
#[derive(Clone, Copy, Debug, Default)]
struct With<'a> {
    /// The index that the hook sees as the staged files.
    index: Option<&'a Path>,
    /// The file that the hook reads as its standard input.
    input: Option<&'a Path>,
}

impl Hooks<'_> {
    /// Each hook that would run, as a line for the person to accept, with the digest of its
    /// file: `hook pre-commit`.
    #[must_use]
    pub fn entries(&self) -> BTreeMap<String, String> {
        let entry = |(name, (_, digest)): (&&str, &(PathBuf, String))| {
            (format!("hook {name}"), digest.clone())
        };
        self.found.iter().map(entry).collect()
    }

    /// A file for a hook, in the Git folder of the runner: no agent of a different thread can
    /// put a link at its place.
    fn file(&self, name: &str) -> Result<PathBuf, GitRefusal> {
        let folder = self.worktree.record().own.join("hooks");
        super::super::place::private_folder(&folder).map_err(|failed| unwritten(&failed))?;
        Ok(folder.join(name))
    }

    /// A copy of the checked `bytes` of the hook `name`, under its own name in a folder of the
    /// Git folder of the runner, that only this user can run. Only the file of the hook is
    /// checked and copied: what it starts or reads is not. Its interpreter is found on the
    /// `PATH` of the login; a file that it reads from the worktree, or from the shared folder
    /// of the hooks, can be changed by any thread of the repository, also a confined one, and
    /// the hook reads it as it is then. A hook that reads files beside itself (`$0`, as husky
    /// does) does not find them beside the copy, and refuses with the words of its shell.
    /// Each run has a folder of its own, so two runs of one hook never share a file; the
    /// caller deletes it after the run ([`Self::drop_copy`]).
    fn copy(&self, name: &str, bytes: &[u8]) -> Result<PathBuf, GitRefusal> {
        use std::io::Write as _;
        #[cfg(unix)]
        use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
        let runs = self.file("run")?;
        super::super::place::private_folder(&runs).map_err(|failed| unwritten(&failed))?;
        own_folder(&runs)?;
        let folder = runs.join(Uuid::now_v7().to_string());
        let mut made = std::fs::DirBuilder::new();
        #[cfg(unix)]
        made.mode(0o700);
        made.create(&folder).map_err(|failed| unwritten(&failed))?;
        let copy = folder.join(name);
        let mut open = std::fs::OpenOptions::new();
        open.create_new(true).write(true);
        #[cfg(unix)]
        open.mode(0o700);
        open.open(&copy)
            .and_then(|mut file| file.write_all(bytes))
            .map_err(|failed| unwritten(&failed))?;
        Ok(copy)
    }

    /// Deletes the copy of a run and its folder; a copy that is already gone is no fault.
    fn drop_copy(copy: &Path) {
        let _ = std::fs::remove_file(copy);
        if let Some(folder) = copy.parent() {
            let _ = std::fs::remove_dir(folder);
        }
    }

    /// Runs the hook `name` when the repository has it and its file is still the one that the
    /// person accepted. A hook that ends with a code that is not 0 refuses the action; its
    /// words are the reason.
    async fn run(&self, name: &str, args: &[&Path], with: With<'_>) -> Result<(), GitRefusal> {
        let Some((file, accepted)) = self.found.get(name) else {
            return Ok(());
        };
        let refused = |why: &str| GitRefusal::Failed {
            said: format!("the hook {name} of the repository refused: {why}"),
        };
        let bytes = runnable(file).unwrap_or_default();
        if crate::model::fetch::hex(&Sha256::digest(&bytes)) != *accepted {
            return Err(refused("its file changed while the action ran"));
        }
        // The bytes that were checked run, from a copy that no agent can change; the file of
        // the repository can change after the check.
        let program_of_run = self.copy(name, &bytes)?;
        let run = ThreadRun {
            kind: Kind::Hook,
            thread: self.worktree.thread(),
            dir: self.worktree.folder().to_owned(),
            program: program_of_run.clone(),
            args: args.iter().map(|arg| arg.as_os_str().to_owned()).collect(),
            env: with
                .index
                .map(|index| ("GIT_INDEX_FILE", index.as_os_str().to_owned()))
                .into_iter()
                .collect(),
            input: with.input.map(Path::to_owned),
            within: HOOK_TIME,
        };
        let ended = self.start.start(run).await;
        Self::drop_copy(&program_of_run);
        match ended {
            Ok(Ended { code: Some(0), .. }) => Ok(()),
            Ok(ended) => Err(refused(&said::without_credentials(&ended.said))),
            Err(failed) => Err(refused(&failed.to_string())),
        }
    }

    /// Runs `pre-commit` and `commit-msg` for `commit`, a commit that no branch names yet and
    /// whose objects the repository has. `pre-commit` sees its tree as the staged files. Gives
    /// the message as `commit-msg` left it.
    ///
    /// # Errors
    /// The refusal of a hook, or a failure to write a file for it.
    pub async fn before_commit(
        &self,
        git: &Git,
        commit: &str,
        message: &str,
    ) -> Result<String, GitRefusal> {
        let index = self.file("index")?;
        let _ = std::fs::remove_file(&index);
        let staged = Run::new(self.worktree.folder(), ["read-tree", commit]);
        git.output(staged.with_env("GIT_INDEX_FILE", &index))
            .await?;
        let with = With {
            index: Some(&index),
            input: None,
        };
        self.run("pre-commit", &[], with).await?;
        let text = self.file("COMMIT_MSG")?;
        std::fs::write(&text, format!("{message}\n")).map_err(|failed| unwritten(&failed))?;
        self.run("commit-msg", &[&text], With::default()).await?;
        let left = std::fs::read(&text).map_err(|failed| unwritten(&failed))?;
        Ok(String::from_utf8_lossy(&left).trim().to_owned())
    }

    /// Runs `post-commit`. What it answers changes nothing: the commit is made.
    pub async fn after_commit(&self) {
        // As git: the code of this hook is not read.
        let _ = self.run("post-commit", &[], With::default()).await;
    }

    /// Runs `pre-push` for a push of `commit` to `branch` of `remote` at `url`.
    ///
    /// # Errors
    /// The refusal of the hook, or a failure to write the file that it reads.
    pub async fn before_push(
        &self,
        (remote, url): (&str, &str),
        (branch, commit): (&str, &str),
    ) -> Result<(), GitRefusal> {
        let lines = self.file("PRE_PUSH")?;
        let none = "0".repeat(commit.len());
        let line = format!("refs/heads/{branch} {commit} refs/heads/{branch} {none}\n");
        std::fs::write(&lines, line).map_err(|failed| unwritten(&failed))?;
        let with = With {
            index: None,
            input: Some(&lines),
        };
        let args = [Path::new(remote), Path::new(url)];
        self.run("pre-push", &args, with).await
    }
}

/// The refusal for a file of a hook that could not be written.
fn unwritten(failed: &std::io::Error) -> GitRefusal {
    GitRefusal::Failed {
        said: format!("a file for a hook could not be written: {failed}"),
    }
}

/// Refuses a folder for the copies that is a link, is not of this user or that others may use:
/// the copy would be written through it.
fn own_folder(folder: &Path) -> Result<(), GitRefusal> {
    let meta = std::fs::symlink_metadata(folder).map_err(|failed| unwritten(&failed))?;
    #[cfg(unix)]
    let ours = {
        use std::os::unix::fs::MetadataExt as _;
        meta.uid() == rustix::process::getuid().as_raw() && meta.mode().trailing_zeros() >= 6
    };
    #[cfg(not(unix))]
    let ours = true;
    if meta.is_dir() && ours {
        Ok(())
    } else {
        Err(unwritten(&std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "the folder of the copies of the hooks is not a private folder of this user",
        )))
    }
}
