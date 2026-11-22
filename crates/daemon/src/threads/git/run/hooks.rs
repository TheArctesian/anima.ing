//! The hooks of the repository of the person (ruling of the operator): a thread with full
//! access may run them for a commit and a push of the person; a confined thread never does.
//!
//! [`of`] is the one place that decides it. The runner's own `git` cannot run a hook of the
//! repository: it does not read that Git folder ([`super::super::project`]). A hook runs only
//! through [`AsThread`], which the runner gives to the engine and which starts a program with
//! the rights of the thread, not the rights of the runner.
//!
//! I/O: writes the files that a hook reads into the Git folder of the worktree.

use std::ffi::OsString;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use agent_threads::git::GitRefusal;
use uuid::Uuid;

use super::super::place::Worktree;
use super::super::project;
use super::{Git, Run, said};

/// How much the agent of a thread may do on this computer, as the runner set it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Access {
    /// The agent is held to its worktree: no hook of the repository runs for its thread.
    #[default]
    Confined,
    /// The agent runs with the rights of the person: the hooks of the repository may run.
    Full,
}

/// One run of the `git` of a thread: `git <args>` in `dir`, with `env` added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadRun {
    pub thread: Uuid,
    pub dir: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(&'static str, OsString)>,
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
}

impl std::fmt::Debug for AsThread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AsThread")
    }
}

/// The hooks of the repository for one action of the person in a thread with full access.
#[derive(Clone, Copy, Debug)]
pub struct Hooks<'a> {
    start: &'a AsThread,
    worktree: &'a Worktree,
}

/// The hooks that an action of the person runs in the thread of `worktree`: `None`, and so no
/// hook at all, for a confined thread and for a runner that gave no way to start one.
#[must_use]
pub fn of<'a>(
    access: Access,
    start: Option<&'a AsThread>,
    worktree: &'a Worktree,
) -> Option<Hooks<'a>> {
    match access {
        Access::Full => start.map(|start| Hooks { start, worktree }),
        Access::Confined => None,
    }
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
    /// A file for a hook in the Git folder of the worktree.
    fn file(&self, name: &str) -> PathBuf {
        self.worktree.record().git_dir.join(name)
    }

    /// Runs the hook `name` when the repository has it. A hook that ends with a code that is
    /// not 0 refuses the action; its words are the reason.
    async fn run(&self, name: &str, args: &[&Path], with: With<'_>) -> Result<(), GitRefusal> {
        let mut all: Vec<OsString> = ["hook", "run", "--ignore-missing"].map(Into::into).into();
        if let Some(input) = with.input {
            let mut option = OsString::from("--to-stdin=");
            option.push(input);
            all.push(option);
        }
        all.extend([name.into(), "--".into()]);
        all.extend(args.iter().map(|arg| arg.as_os_str().to_owned()));
        let run = ThreadRun {
            thread: self.worktree.thread(),
            dir: self.worktree.folder().to_owned(),
            args: all,
            env: with
                .index
                .map(|index| ("GIT_INDEX_FILE", index.as_os_str().to_owned()))
                .into_iter()
                .collect(),
        };
        let refused = |why: &str| GitRefusal::Failed {
            said: format!("the hook {name} of the repository refused: {why}"),
        };
        match (self.start.0)(run).await {
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
        let files = |failed: std::io::Error| GitRefusal::Failed {
            said: format!("a file for a hook could not be written: {failed}"),
        };
        let index = self.file("index.anima-hook");
        let _ = std::fs::remove_file(&index);
        let staged = Run::new(self.worktree.folder(), ["read-tree", commit]);
        git.output(staged.with_env("GIT_INDEX_FILE", &index))
            .await?;
        let checked = self.run(
            "pre-commit",
            &[],
            With {
                index: Some(&index),
                input: None,
            },
        );
        let checked = checked.await;
        let _ = std::fs::remove_file(&index);
        checked?;
        let text = self.file("ANIMA_COMMIT_MSG");
        let _ = std::fs::remove_file(&text);
        std::fs::write(&text, format!("{message}\n")).map_err(files)?;
        let checked = self.run("commit-msg", &[&text], With::default()).await;
        let left = project::limited(&text, 64 * 1024).map_err(files);
        let _ = std::fs::remove_file(&text);
        checked?;
        Ok(String::from_utf8_lossy(&left?).trim().to_owned())
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
        let lines = self.file("ANIMA_PRE_PUSH");
        let none = "0".repeat(commit.len());
        let line = format!("refs/heads/{branch} {commit} refs/heads/{branch} {none}\n");
        let _ = std::fs::remove_file(&lines);
        std::fs::write(&lines, line).map_err(|failed| GitRefusal::Failed {
            said: format!("a file for a hook could not be written: {failed}"),
        })?;
        let with = With {
            index: None,
            input: Some(&lines),
        };
        let args = [Path::new(remote), Path::new(url)];
        let checked = self.run("pre-push", &args, with).await;
        let _ = std::fs::remove_file(&lines);
        checked
    }
}
