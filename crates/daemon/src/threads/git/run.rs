//! One run of `git` for the runner (THR-050): an absolute program, a list of arguments and no
//! shell, a closed environment, no prompt at a terminal, a limit of time and of output.
//!
//! I/O: this is the only place of the Git stage that starts a process.

mod fixed;
pub mod folder;
pub mod said;
pub mod settings;

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub(super) use fixed::{fixed_configuration, fixed_environment};
use folder::FolderId;

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
    #[error("git ended with an error: {said}")]
    Failed { code: Option<i32>, said: String },
}

/// Who asked for a run: it decides whether the hooks of the repository run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    /// The runner by itself (a worktree, a checkpoint, a diff, a revert): no hook and no program
    /// that the configuration of the repository names (THR-050).
    Runner,
    /// The person, by an action of their own (a commit, a push): their hooks run, as in a
    /// terminal of their own.
    Person,
}

/// The `git` of this computer with the environment it runs in.
#[derive(Clone, Debug)]
pub struct Git {
    program: PathBuf,
    env: Vec<(OsString, OsString)>,
    /// The configuration that turns off the filter programs of one repository
    /// ([`Git::quiet_in`]), for a run of the runner itself.
    filters_off: Vec<(String, String)>,
    /// The configuration of a run for the person: the programs of the person, never the ones
    /// that the repository names ([`settings::for_the_person`]).
    of_the_person: Vec<(String, String)>,
    /// The settings of the repository by name, each with a digest of its values.
    of_the_repository: BTreeMap<String, String>,
    /// A run for the person runs the hooks and the filters of the repository
    /// ([`Git::with_repository_programs`]); off unless the runner says so.
    repository_programs: bool,
    /// The work folder of [`Git::for_worktree`] with what it was when the worktree was made:
    /// a folder that was put in its place gets no run.
    folder: Option<(PathBuf, FolderId)>,
    /// The Git folder, the shared Git folder and the work folder of one worktree
    /// ([`Git::for_worktree`]): with them `git` reads no `.git` file and no `core.worktree`.
    bound: Vec<(&'static str, OsString)>,
}

/// One run: where, for whom, with which arguments.
pub struct Run<'a> {
    pub dir: &'a Path,
    pub asked: Asked,
    pub args: Vec<OsString>,
    /// More environment for this run alone (a private index file).
    pub env: Vec<(&'static str, OsString)>,
    pub input: Option<Vec<u8>>,
    pub time: Duration,
}

impl<'a> Run<'a> {
    /// A run of the runner itself in `dir`.
    pub fn new<S: AsRef<OsStr>>(dir: &'a Path, args: impl IntoIterator<Item = S>) -> Self {
        Self {
            dir,
            asked: Asked::Runner,
            args: args
                .into_iter()
                .map(|arg| arg.as_ref().to_owned())
                .collect(),
            env: Vec::new(),
            input: None,
            time: LOCAL_TIME,
        }
    }

    #[must_use]
    pub fn for_the_person(mut self) -> Self {
        self.asked = Asked::Person;
        self
    }

    #[must_use]
    pub fn with_env(mut self, name: &'static str, value: impl Into<OsString>) -> Self {
        self.env.push((name, value.into()));
        self
    }

    #[must_use]
    pub fn with_input(mut self, input: Vec<u8>) -> Self {
        self.input = Some(input);
        self
    }

    #[must_use]
    pub const fn with_time(mut self, time: Duration) -> Self {
        self.time = time;
        self
    }
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
            filters_off: Vec::new(),
            of_the_person: Vec::new(),
            of_the_repository: BTreeMap::new(),
            repository_programs: false,
            folder: None,
            bound: Vec::new(),
        })
    }

    /// This `git` for the repository at `dir`, with each filter program that the configuration
    /// of the repository or of its worktree names turned off for the runs of the runner itself
    /// (THR-050): a checkpoint, a status, a diff or a revert then starts none. `git add`,
    /// `git write-tree`, `git status` and `git restore` can each start one. A run for the
    /// person gets the programs of the person, not the ones that the repository names.
    ///
    /// # Errors
    /// A [`GitError`] when the configuration cannot be read.
    pub async fn quiet_in(&self, dir: &Path) -> Result<Self, GitError> {
        let listed = self.output(Run::new(dir, settings::LIST)).await?;
        let found = settings::read(&String::from_utf8_lossy(&listed));
        let mut quiet = self.clone();
        quiet.filters_off = settings::filters_off(&found);
        quiet.of_the_person = settings::for_the_person(&found);
        quiet.of_the_repository = settings::of_the_repository(&found);
        Ok(quiet)
    }

    /// This `git` with the hooks and the filters of the repository on or off for the runs for
    /// the person (a commit, a push). Off, a commit runs no hook and no program that the
    /// repository names; a run of the runner itself never does.
    #[must_use]
    pub fn with_repository_programs(mut self, on: bool) -> Self {
        self.repository_programs = on;
        self
    }

    #[must_use]
    pub const fn repository_programs(&self) -> bool {
        self.repository_programs
    }

    /// The settings of the repository of [`Git::quiet_in`] by name, each with a digest of its
    /// values: what the person accepts before a commit or a push.
    #[must_use]
    pub const fn of_the_repository(&self) -> &BTreeMap<String, String> {
        &self.of_the_repository
    }

    /// This `git` held to `worktree`: each run names the Git folder, the shared Git folder and
    /// the work folder that the runner wrote down when it made the worktree, so that a changed
    /// `.git` file or a `core.worktree` cannot send a run to the checkout of the person or to a
    /// different folder (P12). The filters of the repository are off as in [`Git::quiet_in`].
    ///
    /// # Errors
    /// A [`GitError`] when the configuration cannot be read.
    pub async fn for_worktree(&self, worktree: &super::place::Worktree) -> Result<Self, GitError> {
        let record = worktree.record();
        let mut held = self.clone();
        held.bound = vec![
            ("GIT_DIR", record.git_dir.clone().into_os_string()),
            ("GIT_COMMON_DIR", record.common_dir.clone().into_os_string()),
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

    async fn finished(&self, run: Run<'_>) -> Result<(Option<i32>, Vec<u8>, String), GitError> {
        // Directly before the run: the work folder is still the one that the runner made, not
        // a folder or a link that an agent put in its place (P12).
        if let Some((folder, made)) = &self.folder
            && FolderId::of(folder) != Some(*made)
        {
            return Err(GitError::Replaced);
        }
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
            .args(fixed_configuration(run.asked, self.repository_programs))
            .args(&run.args)
            .current_dir(run.dir)
            .env_clear()
            .envs(self.env.iter().map(|(name, value)| (name, value)))
            .envs(fixed_environment())
            .envs(self.bound.iter().map(|(name, value)| (name, value)))
            .envs(self.overrides(run.asked))
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

    /// The configuration a run gets through its environment, which wins over each file and can
    /// name a key with any character in it: the filters of the repository, off, for a run of
    /// the runner itself; the programs of the person for a run for the person.
    fn overrides(&self, asked: Asked) -> Vec<(String, String)> {
        let mut set = match asked {
            Asked::Runner => self.filters_off.clone(),
            Asked::Person => self.of_the_person.clone(),
        };
        if asked == Asked::Person && !self.repository_programs {
            set.extend(self.filters_off.iter().cloned());
        }
        let mut env = vec![("GIT_CONFIG_COUNT".to_owned(), set.len().to_string())];
        for (at, (name, value)) in set.iter().enumerate() {
            env.push((format!("GIT_CONFIG_KEY_{at}"), name.clone()));
            env.push((format!("GIT_CONFIG_VALUE_{at}"), value.clone()));
        }
        env
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
