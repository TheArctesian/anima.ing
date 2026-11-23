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

use std::path::{Path, PathBuf};
use std::sync::PoisonError;
use std::time::{Duration, Instant};

use agent_threads::control::{Outcome, RunState};
use agent_threads::drive::Mode;
use agent_threads::git::{GitAction, GitOutcome};
use uuid::Uuid;

use super::Runner;
use super::launcher::{self, SharedRights};
use super::log::Kind;
use super::socket::Caller;
use crate::config::home::HOME_VARIABLE;
use crate::threads::git::place::records;
use crate::threads::git::run::hooks::Access;
use crate::threads::git::wired::{self, Places, Unwired};
use crate::threads::git::{Caller as Asker, Engine, Target};

/// How often the records are touched, so that the system does not clear them as old.
const TOUCH_EVERY: Duration = Duration::from_hours(24);

/// The engine of the runner, or why it has none, and when its records were last touched.
#[derive(Debug)]
pub(super) struct Gits {
    engine: Result<Engine, Unwired>,
    touched: Option<Instant>,
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
        let gits = Self {
            engine,
            touched: None,
        };
        (rights, gits)
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
