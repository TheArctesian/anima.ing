//! One run of `git` for the runner (THR-050): an absolute program, a list of arguments and no
//! shell, a closed environment, no prompt at a terminal, a limit of time and of output.
//!
//! I/O: this is the only place of the Git stage that starts a process.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
/// [`super::said::without_credentials`] first.
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
    /// The arguments that turn off the filter programs of one repository ([`Git::quiet_in`]).
    filters_off: Vec<OsString>,
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
    /// Finds `git` and keeps the allowed part of `env` (the environment of the person).
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
        })
    }

    /// This `git` for the repository at `dir`, with each filter program that the configuration
    /// of the repository names turned off for the runs of the runner itself (THR-050): a
    /// checkpoint, a status, a diff or a revert then starts none. `git add`, `git write-tree`,
    /// `git status` and `git restore` can each start one. The filters of the person's own
    /// configuration stay, and a run for the person keeps each filter.
    ///
    /// # Errors
    /// A [`GitError`] when the configuration cannot be read.
    pub async fn quiet_in(&self, dir: &Path) -> Result<Self, GitError> {
        let names = [
            "config",
            "--local",
            "--includes",
            "--name-only",
            "--get-regexp",
            r"^filter\..*\.(clean|smudge|process|required)$",
        ];
        // `git config --get-regexp` ends with 1 when no name matches.
        let names = match self.output(Run::new(dir, names)).await {
            Ok(out) => String::from_utf8_lossy(&out).into_owned(),
            Err(GitError::Failed { code: Some(1), .. }) => String::new(),
            Err(other) => return Err(other),
        };
        let mut quiet = self.clone();
        quiet.filters_off.clear();
        for name in names.lines().filter(|name| !name.contains('=')) {
            let off = if name.ends_with(".required") {
                "false"
            } else {
                ""
            };
            quiet.filters_off.push(OsString::from("-c"));
            quiet
                .filters_off
                .push(OsString::from(format!("{name}={off}")));
        }
        Ok(quiet)
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
        let mut command = tokio::process::Command::new(&self.program);
        command
            .args(fixed_configuration(run.asked))
            .args(if run.asked == Asked::Runner {
                self.filters_off.as_slice()
            } else {
                &[]
            })
            .args(&run.args)
            .current_dir(run.dir)
            .env_clear()
            .envs(self.env.iter().map(|(name, value)| (name, value)))
            .envs(fixed_environment())
            .envs(run.env.iter().map(|(name, value)| (name, value)))
            .stdin(if run.input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().map_err(GitError::Start)?;
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
        let (code, out, said) = tokio::time::timeout(run.time, work)
            .await
            .map_err(|_| GitError::TimedOut(run.time.as_secs()))??;
        let out = out.ok_or(GitError::TooLarge)?;
        let said = String::from_utf8_lossy(&said.unwrap_or_default()).into_owned();
        Ok((code, out, super::said::without_credentials(&said)))
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

/// The configuration each run gets on its command line, where it wins over each file: no
/// monitor program, no external diff, no `ext::` transport, no pager, no optional lock, quoted
/// paths as bytes. A run of the runner itself has no hooks.
fn fixed_configuration(asked: Asked) -> Vec<&'static str> {
    let mut args = vec![
        "--no-pager",
        "--no-optional-locks",
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.quotePath=false",
        "-c",
        "core.sshCommand=ssh -o BatchMode=yes",
        "-c",
        "protocol.ext.allow=never",
        "-c",
        "diff.external=",
        "-c",
        "advice.detachedHead=false",
        "-c",
        "gc.auto=0",
    ];
    if asked == Asked::Runner {
        args.extend(["-c", "core.hooksPath=/dev/null"]);
    }
    args
}

/// The environment each run gets after the kept names: no prompt, fixed words, a fixed `ssh`.
fn fixed_environment() -> [(&'static str, &'static str); 5] {
    [
        ("GIT_TERMINAL_PROMPT", "0"),
        ("GIT_SSH_COMMAND", "ssh -o BatchMode=yes"),
        ("GIT_PAGER", "cat"),
        ("LC_ALL", "C"),
        ("GIT_CONFIG_COUNT", "0"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_kept_names_of_the_environment_reach_git() {
        let env = [
            ("HOME", "/home/ada"),
            ("PATH", "/usr/bin"),
            ("GIT_DIR", "/elsewhere/.git"),
            ("GIT_INDEX_FILE", "/elsewhere/index"),
            ("ANIMA_DAEMON_TOKEN", "secret"),
            ("GH_TOKEN", "secret"),
        ]
        .map(|(name, value)| (OsString::from(name), OsString::from(value)));
        let Ok(git) = Git::find(env) else {
            return;
        };
        let names: Vec<_> = git.env.iter().map(|(name, _)| name.clone()).collect();
        assert_eq!(names, ["HOME", "PATH"]);
    }

    #[test]
    fn the_runner_has_no_hooks_and_the_person_has_theirs() {
        assert!(fixed_configuration(Asked::Runner).contains(&"core.hooksPath=/dev/null"));
        assert!(!fixed_configuration(Asked::Person).contains(&"core.hooksPath=/dev/null"));
    }
}
