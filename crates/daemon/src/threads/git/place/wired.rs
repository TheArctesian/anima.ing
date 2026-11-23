//! The engine as the runner makes it at its start (step W1 of TC7): the records of this user
//! outside the home folder, the home of the worktrees, the `git` of the person, and the
//! launcher of the runner. The runner makes one engine, gives a clone to each task, and calls
//! [`Engine::worktree_shown`], [`Engine::turn_ended`], [`Engine::hold`] and [`Engine::call`]; it
//! touches the records each day ([`super::records::touch`]).
//!
//! I/O: makes and checks the folder of the records; finds `git`.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use agent_threads::git::GitRefusal;

use super::super::Engine;
use super::super::run::{Git, GitError, hooks::AsThread};
use super::records::{self, BadRecords};
use super::{BadPlace, Home};

/// Where the engine keeps what it makes.
#[derive(Clone, Debug, Default)]
pub struct Places {
    /// The home of Anima (`~/.anima`): the worktrees are in `worktrees` there, unless the
    /// person chose a folder.
    pub anima: PathBuf,
    /// The folder of the worktrees that the person chose in the settings, when there is one.
    pub chosen: Option<PathBuf>,
    /// The folders that a chosen folder must not be in and must not hold: the home of the
    /// daemon, and each folder that the sandbox of an agent can write.
    pub kept_out: Vec<PathBuf>,
    /// The folder of the records; `None` is the place of this user
    /// ([`records::standard`]).
    pub records: Option<PathBuf>,
    /// The runner gives each agent a `TMPDIR` of its own, never the one of the login (THR-050).
    /// While it does not, records in `/tmp` or in the `TMPDIR` of the login are refused: each
    /// agent can write there.
    pub private_tmp: bool,
}

/// Why the runner has no engine. Each Git action of each thread is then refused with
/// [`Unwired::refusal`].
#[derive(Debug, thiserror::Error)]
pub enum Unwired {
    #[error("not available on Windows yet")]
    NotOnWindows,
    #[error(transparent)]
    Records(#[from] BadRecords),
    #[error("the folder of the worktrees cannot be used: {0}")]
    Home(#[from] BadPlace),
    #[error(transparent)]
    Git(#[from] GitError),
}

impl Unwired {
    /// The answer to each Git action while the runner has no engine.
    #[must_use]
    pub fn refusal(&self) -> GitRefusal {
        match self {
            Self::NotOnWindows => GitRefusal::NotOnWindows,
            Self::Git(GitError::NotFound) => GitRefusal::GitNotFound,
            other => GitRefusal::Failed {
                said: other.to_string(),
            },
        }
    }
}

/// Makes the engine. `login` is the login environment of the person, the one their agents get
/// (THR-040). `launcher` starts the reads and the hooks of a thread with the rights of that
/// thread; the runner gives it for each thread, also a confined one.
///
/// # Errors
/// An [`Unwired`] when a place is refused or `git` is not found.
pub fn engine(
    login: impl IntoIterator<Item = (OsString, OsString)>,
    places: &Places,
    launcher: AsThread,
) -> Result<Engine, Unwired> {
    if cfg!(windows) {
        return Err(Unwired::NotOnWindows);
    }
    let login: Vec<_> = login.into_iter().collect();
    let place = match &places.records {
        Some(place) => place.clone(),
        None => records::standard(this_user()).ok_or(BadRecords::NoPlace)?,
    };
    if !places.private_tmp && in_temp(&place, &login) {
        return Err(BadRecords::InTemp.into());
    }
    let records = records::opened(&place)?;
    let home = home(places, &records)?;
    let git = Git::find(login)?;
    Ok(Engine::new(git, home).with_launcher(launcher))
}

impl Engine {
    /// The `git` that this engine runs, and that each read of a thread names: the launcher
    /// checks a read against it ([`super::shared::pack::is_read`]).
    #[must_use]
    pub fn program(&self) -> &Path {
        self.git.program()
    }

    /// The folder of the records of this engine: the list of a pack read is in it, and the
    /// runner touches it each day and checks that no thread runs in a folder that holds it.
    #[must_use]
    pub fn records_folder(&self) -> &Path {
        self.home.records()
    }
}

/// The names of the login environment that name a temporary folder of each program of the
/// person, and so of each agent while agents get them from the login.
const TEMPS: [&str; 4] = ["TMPDIR", "TMP", "TEMP", "XDG_RUNTIME_DIR"];

/// The records at `records` are in a temporary folder that each agent can write while agents
/// get the ones of the login: `/tmp`, or each folder that `login` names in [`TEMPS`]. A name
/// that is not a full path counts as one that holds them: each agent reads it from its own
/// folder.
fn in_temp(records: &Path, login: &[(OsString, OsString)]) -> bool {
    let temps: Vec<_> = login
        .iter()
        .filter(|(name, value)| TEMPS.iter().any(|temp| name == temp) && !value.is_empty())
        .map(|(_, value)| PathBuf::from(value))
        .chain(std::iter::once(PathBuf::from("/tmp")))
        .collect();
    temps.iter().any(|temp| !temp.is_absolute()) || records::held_by_any(records, &temps)
}

/// The home of the worktrees: the one the person chose, or the standard one.
fn home(places: &Places, records: &Path) -> Result<Home, BadPlace> {
    match &places.chosen {
        Some(chosen) => Home::chosen(chosen, records, &places.kept_out),
        None => Home::standard(&places.anima, records),
    }
}

#[cfg(all(test, unix))]
mod tests;

#[cfg(unix)]
fn this_user() -> u32 {
    records::this_user()
}

#[cfg(not(unix))]
const fn this_user() -> u32 {
    0
}
