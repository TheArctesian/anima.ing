//! The Git engine of the runner (TC7 step W1, TH116): the runner makes it at its start with
//! its launcher, keeps [`Engine::hold`] true while a thread runs in a folder that holds the
//! records, touches the records once a day, and does each Git call of a client in a task of its
//! own, because a commit with hooks or a push can take minutes.
//!
//! Where the records are: the place of this user outside the home folder
//! ([`records::standard`]), or, for a daemon with a home folder of its own (`ANIMA_DAEMON_HOME`,
//! as each test sets), the folder `git-records` of its threads, so that it never shares the
//! records of the installed daemon. The worktrees are in `~/.anima/worktrees` of the login.
//!
//! I/O: makes the engine and its records folder; starts the tasks of the calls.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::PoisonError;
use std::time::{Duration, Instant};

use agent_threads::control::{Outcome, RunState};
use agent_threads::drive::Mode;
use agent_threads::git::{GitAction, GitOutcome, GitRefusal};
use uuid::Uuid;

use super::Runner;
use super::launcher::{self, SharedRights};
use super::log::Kind;
use super::socket::Caller;
use crate::config::home::HOME_VARIABLE;
use crate::threads::git::place::{Home, records};
use crate::threads::git::run::hooks::Access;
use crate::threads::git::wired::{self, Places, Unwired};
use crate::threads::git::{Caller as Asker, Engine, Target};

/// How often the records are touched, so that the system does not clear them as old.
const TOUCH_EVERY: Duration = Duration::from_hours(24);

/// The engine of the runner, or why it has none, and when its records were last touched.
#[derive(Debug)]
pub(super) struct Gits {
    engine: Result<Engine, Unwired>,
    /// The home of the worktrees of the engine, to know the folder of a thread.
    home: Option<Home>,
    touched: Option<Instant>,
    /// One checkpoint of a thread at a time, so that two ends of a turn never take one number.
    turns: BTreeMap<Uuid, Arc<tokio::sync::Mutex<()>>>,
}

impl Gits {
    /// The rights of the launcher and the engine for the login `env` of the agents, which starts
    /// the runs of a thread with that launcher; the rights get its `git` and its records.
    /// `daemon_env` says whether the daemon has a home folder of its own; `dir` is the folder of
    /// its threads.
    pub(super) fn make(
        env: &[(String, String)],
        daemon_env: &[(String, String)],
        dir: &Path,
    ) -> (SharedRights, Self) {
        let rights = SharedRights::default();
        let home = env
            .iter()
            .find(|(name, _)| name == "HOME")
            .map(|(_, home)| PathBuf::from(home));
        let own_home = daemon_env.iter().any(|(name, _)| name == HOME_VARIABLE);
        let places = Places {
            anima: home.unwrap_or_default().join(".anima"),
            chosen: None,
            kept_out: dir.parent().map(Path::to_owned).into_iter().collect(),
            // The real path: the records refuse a link above them, and `/var` of macOS is one.
            records: own_home.then(|| {
                std::fs::canonicalize(dir)
                    .unwrap_or_else(|_| dir.to_owned())
                    .join("git-records")
            }),
            // Each agent gets a `TMPDIR` of its own, never the one of the login (THR-050,
            // [`super::start::private_tmp`]).
            private_tmp: true,
        };
        let login = env.iter().map(|(name, value)| (name.into(), value.into()));
        let engine = wired::engine(login, &places, launcher::launcher(rights.clone()));
        match &engine {
            Ok(engine) => {
                let mut seen = rights.lock().unwrap_or_else(PoisonError::into_inner);
                seen.git = Some(engine.program().to_owned());
                seen.records = Some(engine.records_folder().to_owned());
            }
            Err(error) => {
                tracing::warn!(%error, "the runner has no Git engine; each Git call is refused");
            }
        }
        let home = engine
            .as_ref()
            .ok()
            .and_then(|engine| Home::standard(&places.anima, engine.records_folder()).ok());
        let gits = Self {
            engine,
            home,
            touched: None,
            turns: BTreeMap::new(),
        };
        (rights, gits)
    }

    /// The engine, or the answer to each Git action while there is none.
    pub(super) fn engine(&self) -> Result<&Engine, GitRefusal> {
        self.engine.as_ref().map_err(Unwired::refusal)
    }

    /// The folder that holds the worktrees of `thread`.
    pub(super) fn thread_folder(&self, thread: Uuid) -> Option<PathBuf> {
        Some(self.home.as_ref()?.of_thread(thread))
    }

    pub(super) fn read_engine(&self) -> Option<Engine> {
        self.engine.as_ref().ok().cloned()
    }

    /// The repository of the worktree that `folder` is, when it is a worktree of `thread`.
    pub(super) fn repository_of(&self, thread: Uuid, folder: &Path) -> Option<String> {
        let real = |path: &Path| std::fs::canonicalize(path).ok();
        let above = real(folder.parent()?)?;
        (Some(above) == real(&self.thread_folder(thread)?))
            .then(|| folder.file_name()?.to_str().map(str::to_owned))
            .flatten()
    }
}

/// The word of the runner log for an action that changes something (THR-044); `None` for a
/// read.
const fn logged(action: &GitAction) -> Option<&'static str> {
    match action {
        GitAction::Status | GitAction::Checkpoints | GitAction::Changes { .. } => None,
        GitAction::Revert { .. } => Some("git revert"),
        GitAction::Commit { .. } => Some("git commit"),
        GitAction::Push { .. } => Some("git push"),
        GitAction::PullRequest => Some("git pull request"),
        GitAction::Remove => Some("git remove"),
        GitAction::Forget { .. } => Some("git forget"),
        GitAction::Trust { .. } => Some("git trust"),
    }
}

impl Runner {
    /// Does a Git call of a client in a task of its own and answers it there. `agent` is what
    /// the runner saw of the caller, never a field of the frame. The runner does not know which
    /// thread an agent belongs to, so the engine refuses each call of an agent.
    pub(super) fn git_call(
        &mut self,
        (thread, repository, action): (Uuid, String, GitAction),
        agent: bool,
        back: Caller,
    ) {
        let engine = match &self.git.engine {
            Ok(engine) => engine.clone(),
            Err(unwired) => {
                let why = unwired.refusal();
                back.answer(Outcome::Git {
                    result: GitOutcome::Refused { why },
                });
                return;
            }
        };
        let row = self.threads.get(&thread).map(|thread| &thread.row);
        let running = row.is_some_and(|row| {
            matches!(
                row.state,
                RunState::Starting | RunState::Working | RunState::NeedsYou
            )
        });
        let access = match row.map(|row| row.mode) {
            Some(Mode::FullAccess) => Access::Full,
            _ => Access::Confined,
        };
        if let Some(word) = logged(&action).filter(|_| !agent) {
            self.note(Kind::Git, Some(thread), word);
        }
        let asker = Asker {
            agent,
            running,
            thread: None,
            access,
        };
        tokio::spawn(async move {
            let target = Target {
                thread,
                repository: &repository,
            };
            let result = engine.call(asker, target, action).await;
            back.answer(Outcome::Git { result });
        });
    }

    /// Tells the engine whether a thread runs in a folder that holds its records.
    pub(super) fn git_hold(&self) {
        if let Ok(engine) = &self.git.engine {
            let rights = self.rights.lock().unwrap_or_else(PoisonError::into_inner);
            engine.hold(rights.hold(engine.records_folder()));
        }
    }

    /// Takes the checkpoint of the turn of `thread` that ended, away from the loop, when the
    /// thread runs in a worktree of the runner. Its number is one more than the last
    /// checkpoint, so a restart of the runner goes on where it was.
    pub(super) fn git_turn_ended(&mut self, thread: Uuid) {
        let (Ok(engine), Some(row)) = (&self.git.engine, self.threads.get(&thread)) else {
            return;
        };
        let Some(repository) = self.git.repository_of(thread, &row.row.folder) else {
            return;
        };
        let engine = engine.clone();
        let one_at_a_time = Arc::clone(self.git.turns.entry(thread).or_default());
        tokio::spawn(async move {
            let _alone = one_at_a_time.lock().await;
            let target = Target {
                thread,
                repository: &repository,
            };
            let person = Asker {
                agent: false,
                running: false,
                thread: None,
                access: Access::Confined,
            };
            let last = match engine.call(person, target, GitAction::Checkpoints).await {
                GitOutcome::Checkpoints { checkpoints } => {
                    checkpoints.iter().map(|checkpoint| checkpoint.turn).max()
                }
                _ => None,
            };
            let turn = last.map_or(1, |last| last.saturating_add(1));
            // The answer of `git` can hold text of the repository: only the fact is logged.
            if engine.turn_ended(target, turn).await.is_err() {
                tracing::warn!(%thread, turn, "the checkpoint of a turn was not taken");
            }
        });
    }

    /// Touches the records once a day, away from the loop.
    pub(super) fn git_touch(&mut self) {
        let Ok(engine) = &self.git.engine else {
            return;
        };
        let due = self
            .git
            .touched
            .is_none_or(|touched| touched.elapsed() >= TOUCH_EVERY);
        if !due {
            return;
        }
        self.git.touched = Some(Instant::now());
        let place = engine.records_folder().to_owned();
        tokio::task::spawn_blocking(move || {
            if let Err(error) = records::touch(&place) {
                tracing::warn!(%error, "the records of the Git engine were not touched");
            }
        });
    }
}
