//! One run of `git` for the runner (THR-050): an absolute program, a list of arguments and no
//! shell, a closed environment, no prompt at a terminal, a limit of time and of output.
//!
//! I/O: this is the only place of the Git stage that starts a process.

mod fixed;
pub mod folder;
pub mod hooks;
mod one;
mod records;
pub mod said;
pub mod settings;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) use fixed::{fixed_configuration, fixed_environment};
use folder::FolderId;
pub use one::Run;

/// The most bytes of output one run keeps; a run that writes more is ended and refused.
pub const MAX_OUTPUT: usize = 8 * 1024 * 1024;

/// The time a run of the runner itself gets (a status, a checkpoint, a diff).
pub const LOCAL_TIME: Duration = Duration::from_secs(60);

/// The time a run that speaks to a remote gets (a push).
pub const REMOTE_TIME: Duration = Duration::from_secs(300);

/// The names of the environment of the person that `git` gets. Each other name is dropped, so
/// no `GIT_DIR`, `GIT_INDEX_FILE` or `GIT_CONFIG_*` of a caller reaches it.
const KEPT: [&str; 9] = [
    "HOME",
    "PATH",
    "USER",
    "LOGNAME",
    "TMPDIR",
    "SSH_AUTH_SOCK",
    "XDG_CONFIG_HOME",
    "XDG_RUNTIME_DIR",
    "SYSTEMROOT",
];

/// The places where `git` is looked for, in this order, before the `PATH` of the person.
const PLACES: [&str; 3] = [
    "/usr/bin/git",
    "/opt/homebrew/bin/git",
    "/usr/local/bin/git",
];

/// Why a run gave no result. No variant holds a credential: the words of `git` pass
/// [`said::without_credentials`] first.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("git is not installed on this computer")]
    NotFound,
    #[error("git did not start: {0}")]
    Start(#[source] std::io::Error),
    #[error("git did not end in {0} seconds")]
    TimedOut(u64),
    #[error("git wrote more than {MAX_OUTPUT} bytes")]
    TooLarge,
    #[error("the folder of the worktree was replaced; nothing ran in it")]
    Replaced,
    #[error("a file of the runner could not be written: {0}")]
    Files(#[source] std::io::Error),
    #[error("git ended with an error: {said}")]
    Failed { code: Option<i32>, said: String },
}

/// Who asked for a run. No run reads the configuration or the hooks of the repository of the
/// person (THR-050); this decides only what of the person's own applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    /// The runner by itself (a worktree, a checkpoint, a diff, a revert): no hook at all, and
    /// no transport.
    Runner,
    /// The person, by an action of their own (a commit, a push): the hooks, the key and the
    /// credentials that they set for all their repositories apply.
    Person,
}

/// The `git` of this computer with the environment it runs in.
#[derive(Clone, Debug)]
pub struct Git {
    program: PathBuf,
    env: Vec<(OsString, OsString)>,
    /// Each hook that the configuration names, off ([`settings::hooks_off`]).
    hooks_off: Vec<(String, String)>,
    /// The person signs their commits ([`settings::signs`]).
    signs: bool,
    /// A push to a folder of this computer is allowed; only the tests have such a remote.
    local_remotes: bool,
    /// The bytes that the checked copy of a project can hold besides three times its first
    /// size (`place::shared`).
    copy_room: u64,
    /// The bytes that the Git folder of the runner for one worktree can hold
    /// (`place::own::within`).
    own_room: u64,
    /// How a program is started with the rights of a thread ([`hooks::AsThread`]): the only
    /// way the repository of the person is read.
    as_thread: Option<hooks::AsThread>,
    /// The work folder of [`Git::for_worktree`] with what it was when the worktree was made:
    /// a folder that was put in its place gets no run.
    folder: Option<(PathBuf, FolderId)>,
    /// The Git folder of the runner and the work folder of one worktree
    /// ([`Git::for_worktree`]): with them `git` reads no `.git` file of the worktree and
    /// nothing of the Git folder of the person but its objects.
    bound: Vec<(&'static str, OsString)>,
    records: Option<PathBuf>,
}

impl Git {
    /// Finds `git` and keeps the allowed part of `env`. `env` is the login environment of the
    /// person (THR-040), the one their agents get, not the environment of the daemon: the
    /// `PATH`, the `HOME` and the `SSH_AUTH_SOCK` of the person are the ones that find their
    /// `git`, their credential helper and their keys.
    ///
    /// # Errors
    /// [`GitError::NotFound`] when no `git` is at a known place or in the `PATH` of `env`.
    pub fn find(env: impl IntoIterator<Item = (OsString, OsString)>) -> Result<Self, GitError> {
        let env: Vec<(OsString, OsString)> = env
            .into_iter()
            .filter(|(name, _)| KEPT.iter().any(|kept| name == kept))
            .collect();
        let in_path: Vec<PathBuf> = env
            .iter()
            .find(|(name, _)| name == "PATH")
            .map_or_else(Vec::new, |(_, path)| std::env::split_paths(path).collect());
        let program = PLACES
            .iter()
            .map(PathBuf::from)
            .chain(in_path.into_iter().map(|dir| dir.join("git")))
            .find(|place| place.is_absolute() && place.is_file())
            .ok_or(GitError::NotFound)?;
        Ok(Self {
            program,
            env,
            hooks_off: Vec::new(),
            signs: false,
            local_remotes: false,
            copy_room: super::place::shared::ROOM,
            own_room: super::place::own::ROOM,
            as_thread: None,
            folder: None,
            bound: Vec::new(),
            records: None,
        })
    }

    /// This `git` with each hook that the configuration of the person names turned off for
    /// the runs of the runner itself (THR-050). `dir` is where `git` reads the configuration:
    /// the folder of the records, or a worktree with the Git folder of the runner.
    ///
    /// # Errors
    /// A [`GitError`] when the configuration cannot be read.
    pub async fn quiet_in(&self, dir: &Path) -> Result<Self, GitError> {
        let listed = self.output(Run::new(dir, settings::LIST)).await?;
        // A name that is not text cannot be turned off by name, so nothing runs with it.
        let listed = String::from_utf8(listed).map_err(|_| GitError::Failed {
            code: None,
            said: "the configuration has a name or a value that is not text".to_owned(),
        })?;
        let found = settings::read(&listed);
        let mut quiet = self.clone();
        quiet.hooks_off = settings::hooks_off(&found);
        quiet.signs = settings::signs(&found);
        Ok(quiet)
    }

    /// This `git` with a different size that the copy of a project can always have (a
    /// setting of the person; the tests use a small one).
    #[must_use]
    pub const fn with_copy_room(mut self, bytes: u64) -> Self {
        self.copy_room = bytes;
        self
    }

    /// This `git` with a different size for the checkpoints of one thread (a setting of the
    /// person; the tests use a small one).
    #[must_use]
    pub const fn with_own_room(mut self, bytes: u64) -> Self {
        self.own_room = bytes;
        self
    }

    /// This `git` saves the work of a worktree that is removed: no size holds it back, and
    /// each file of the worktree is taken, also in a repository inside it.
    #[must_use]
    pub const fn removing(&self) -> bool {
        self.own_room == u64::MAX
    }

    /// The bytes that the checkpoints of one thread can use.
    #[must_use]
    pub const fn own_room(&self) -> u64 {
        self.own_room
    }

    /// The bytes that the copy of a project can always have.
    #[must_use]
    pub const fn copy_room(&self) -> u64 {
        self.copy_room
    }

    /// This `git` with the way to start a program as the agent of a thread is started.
    #[must_use]
    pub fn with_thread(mut self, as_thread: hooks::AsThread) -> Self {
        self.as_thread = Some(as_thread);
        self
    }

    /// How a program is started with the rights of a thread, when the runner gave a way.
    #[must_use]
    pub const fn as_thread(&self) -> Option<&hooks::AsThread> {
        self.as_thread.as_ref()
    }

    /// The `git` program of this computer.
    #[must_use]
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// A push to a folder of this computer is allowed (only in the tests).
    #[must_use]
    pub const fn local_remotes(&self) -> bool {
        self.local_remotes
    }

    /// The person signs their commits.
    #[must_use]
    pub const fn signs(&self) -> bool {
        self.signs
    }

    /// This `git` with a push to a folder of this computer allowed, for the tests: no test
    /// speaks to a network, so their only remote is a folder.
    #[cfg(test)]
    #[must_use]
    pub(super) fn with_local_remotes(mut self) -> Self {
        self.local_remotes = true;
        self
    }

    /// This `git` held to no worktree and no Git folder: to make a new Git folder of the runner.
    #[must_use]
    pub fn unbound(&self) -> Self {
        let mut free = self.clone();
        free.bound = Vec::new();
        free.folder = None;
        free
    }

    /// This `git` with the Git folder of the runner at `own` and no work folder, for the
    /// moment before a worktree has its files: no run looks for a repository above a folder.
    ///
    /// # Errors
    /// A [`GitError`] when the configuration of the person cannot be read.
    pub async fn in_own(&self, own: &Path) -> Result<Self, GitError> {
        let mut held = self.clone();
        held.bound = vec![("GIT_DIR", own.as_os_str().to_owned())];
        held.quiet_in(own).await
    }

    /// This `git` held to `worktree`: each run has the Git folder of the runner for it and
    /// its work folder. The `.git` file of the worktree, the configuration, the hooks and the
    /// `info` folder of the repository of the person are not read, whatever an agent wrote
    /// there (P12, THR-050).
    ///
    /// # Errors
    /// A [`GitError`] when the configuration of the person cannot be read.
    pub async fn for_worktree(&self, worktree: &super::place::Worktree) -> Result<Self, GitError> {
        let record = worktree.record();
        let mut held = self.clone();
        held.bound = vec![
            ("GIT_DIR", record.own.clone().into_os_string()),
            ("GIT_WORK_TREE", record.folder.clone().into_os_string()),
        ];
        held.folder = record.folder_id.map(|id| (record.folder.clone(), id));
        held.quiet_in(&record.folder).await
    }

    /// Runs `git` and gives what it wrote to standard output.
    ///
    /// # Errors
    /// A [`GitError`] when `git` did not start, did not end in time, wrote too much, or ended
    /// with a code that is not 0.
    pub async fn output(&self, run: Run<'_>) -> Result<Vec<u8>, GitError> {
        let (code, out, said) = self.finished(run).await?;
        if code == Some(0) {
            Ok(out)
        } else {
            Err(GitError::Failed { code, said })
        }
    }

    /// As [`Git::output`], as text with the last line end removed.
    ///
    /// # Errors
    /// As [`Git::output`].
    pub async fn text(&self, run: Run<'_>) -> Result<String, GitError> {
        let out = self.output(run).await?;
        Ok(String::from_utf8_lossy(&out).trim_end().to_owned())
    }

    /// Runs `git` for a question whose answer is its exit code: 0 is yes, 1 is no.
    ///
    /// # Errors
    /// A [`GitError`] for each other code and each failure of the run.
    pub async fn yes(&self, run: Run<'_>) -> Result<bool, GitError> {
        match self.finished(run).await? {
            (Some(0), ..) => Ok(true),
            (Some(1), ..) => Ok(false),
            (code, _, said) => Err(GitError::Failed { code, said }),
        }
    }

    pub(super) async fn finished(
        &self,
        run: Run<'_>,
    ) -> Result<(Option<i32>, Vec<u8>, String), GitError> {
        // Directly before the run: the work folder is still the one that the runner made, not
        // a folder or a link that an agent put in its place (P12).
        if let Some((folder, made)) = &self.folder
            && FolderId::of(folder) != Some(*made)
        {
            return Err(GitError::Replaced);
        }
        self.check_records(&run)?;
        let mut child = self.command(&run).spawn().map_err(GitError::Start)?;
        let group = child.id();
        let input = child.stdin.take().zip(run.input);
        let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
        let work = async {
            let write = async {
                if let Some((mut pipe, bytes)) = input {
                    // A `git` that ends before it read all of the input is told by its code.
                    let _ = pipe.write_all(&bytes).await;
                }
            };
            let ((), out, said) = tokio::join!(write, held(stdout), held(stderr));
            let status = child.wait().await.map_err(GitError::Start)?;
            Ok::<_, GitError>((status.code(), out, said))
        };
        let done = tokio::time::timeout(run.time, work).await;
        let whole = matches!(&done, Ok(Ok((_, Some(_), _))));
        if !whole {
            // A hook or a helper that `git` started ends with it, not only `git` itself.
            end_group(group);
        }
        let (code, out, said) = done.map_err(|_| GitError::TimedOut(run.time.as_secs()))??;
        let out = out.ok_or(GitError::TooLarge)?;
        let said = String::from_utf8_lossy(&said.unwrap_or_default()).into_owned();
        Ok((code, out, said::without_credentials(&said)))
    }

    /// The process of one run, in a process group of its own.
    fn command(&self, run: &Run<'_>) -> tokio::process::Command {
        let mut command = tokio::process::Command::new(&self.program);
        command
            .args(fixed_configuration(run.asked))
            .args(&run.args)
            .current_dir(run.dir)
            .env_clear()
            .envs(self.env.iter().map(|(name, value)| (name, value)))
            .envs(fixed_environment())
            .envs(self.bound.iter().map(|(name, value)| (name, value)))
            .envs(self.overrides(run))
            .envs(run.env.iter().map(|(name, value)| (name, value)))
            .stdin(if run.input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        command
    }
}

/// Ends each process of the group of a run that did not end well by itself.
#[cfg(unix)]
fn end_group(group: Option<u32>) {
    let pid = group
        .and_then(|pid| i32::try_from(pid).ok())
        .and_then(rustix::process::Pid::from_raw);
    if let Some(pid) = pid {
        // A group that ended already has nothing to end.
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
}

/// On Windows `kill_on_drop` ends `git`; a process group is a thing of Unix.
#[cfg(not(unix))]
const fn end_group(_group: Option<u32>) {}

impl Git {
    /// The names of the environment that reach `git`, for the tests.
    #[cfg(test)]
    pub(super) fn env_names(&self) -> Vec<OsString> {
        self.env.iter().map(|(name, _)| name.clone()).collect()
    }
}

/// Reads a pipe to its end; `None` when it gave more than [`MAX_OUTPUT`].
async fn held(pipe: Option<impl tokio::io::AsyncRead + Unpin>) -> Option<Vec<u8>> {
    let mut kept = Vec::new();
    let Some(pipe) = pipe else {
        return Some(kept);
    };
    let limit = u64::try_from(MAX_OUTPUT).unwrap_or(u64::MAX);
    let mut pipe = pipe.take(limit.saturating_add(1));
    // A pipe that fails gives what it gave so far; the exit code tells the rest.
    let _ = pipe.read_to_end(&mut kept).await;
    (kept.len() <= MAX_OUTPUT).then_some(kept)
}
