//! The launcher of the runner: it starts one program for a thread with the rights of that
//! thread, for the Git engine ([`AsThread`]), so a hook of a repository never runs with the
//! rights of the runner (THR-050).
//!
//! A program runs only for a thread of this runner that runs in Full access, so with the rights
//! that its agent has already. A confined thread runs nothing: the engine runs no hook for it
//! ([`crate::threads::git::run::hooks::of`]), and this launcher refuses it again, so no change
//! of the engine can widen it. The program gets the environment of the login that the agents
//! get, with no name of Anima (THR-040), the mark of its thread (THR-049) and only the names
//! of the Git engine that [`ADDED`] lists. It runs in a process group of its own, which ends
//! at its time limit and when the run is dropped.
//!
//! I/O: starts the program and reads its standard error.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, PoisonError};

use agent_threads::control::RunState;
use agent_threads::drive::Mode;
use tokio::io::AsyncReadExt as _;
use uuid::Uuid;

use super::Runner;
use crate::threads::git::place::records::held_by;
use crate::threads::git::run::hooks::{AsThread, Ended, ThreadRun};
use crate::threads::marker;

/// The names that the Git engine may add to the environment of a run.
const ADDED: [&str; 1] = ["GIT_INDEX_FILE"];

/// The most bytes of the standard error of a run that are kept.
const MOST_SAID: usize = 64 * 1024;

/// How long the standard error is read after the program ended: a process that the program left
/// in the background can keep it open.
const SAID_AFTER_END: std::time::Duration = std::time::Duration::from_secs(1);

/// What the launcher knows of the threads that run: the runner writes it, the launcher reads it.
#[derive(Debug, Default)]
pub struct Rights {
    /// The environment of the login that the agents get.
    pub(super) env: Vec<(String, String)>,
    /// Each thread that runs: its mode and its folder.
    pub(super) threads: BTreeMap<Uuid, (Mode, PathBuf)>,
}

/// The rights, shared by the runner and each clone of its launcher.
pub type SharedRights = Arc<Mutex<Rights>>;

impl Rights {
    /// Whether a thread that runs is in a folder that holds `records`: its agent can write them
    /// ([`crate::threads::git::Engine::hold`]).
    #[must_use]
    pub fn hold(&self, records: &Path) -> bool {
        self.threads
            .values()
            .any(|(_, folder)| held_by(records, folder))
    }

    /// The environment of `run`, after the checks of its rights.
    fn checked(&self, run: &ThreadRun) -> io::Result<(PathBuf, Vec<(OsString, OsString)>)> {
        let Some((mode, folder)) = self.threads.get(&run.thread) else {
            return Err(denied("the thread does not run in this runner"));
        };
        if *mode != Mode::FullAccess {
            return Err(denied(
                "a thread that is not in Full access runs no program",
            ));
        }
        if !run.program.is_absolute() {
            return Err(denied("the program is not named by its full path"));
        }
        let dir = std::fs::canonicalize(&run.dir)?;
        if !dir.starts_with(std::fs::canonicalize(folder)?) {
            return Err(denied(
                "the folder of the run is not in the folder of the thread",
            ));
        }
        if let Some((name, _)) = run.env.iter().find(|(name, _)| !ADDED.contains(name)) {
            return Err(denied(&format!(
                "the name {name} is not one the engine may add"
            )));
        }
        let login = self
            .env
            .iter()
            .filter(|(name, _)| !name.starts_with("ANIMA_") && !ADDED.contains(&name.as_str()))
            .map(|(name, value)| (OsString::from(name), OsString::from(value)));
        let added = run
            .env
            .iter()
            .map(|(name, value)| (OsString::from(name), value.clone()));
        let (mark, key) = marker::entry(&run.thread.to_string());
        let env = login
            .chain(added)
            .chain([(OsString::from(mark), OsString::from(key))])
            .collect();
        Ok((dir, env))
    }
}

fn denied(why: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, why.to_owned())
}

/// The launcher that the runner gives to the Git engine.
#[must_use]
pub fn launcher(rights: SharedRights) -> AsThread {
    AsThread::new(move |run| {
        let rights = Arc::clone(&rights);
        Box::pin(async move { start(&rights, run).await })
    })
}

/// Starts `run` after the checks of its rights, and waits for its end or its time limit.
pub(super) async fn start(rights: &SharedRights, run: ThreadRun) -> io::Result<Ended> {
    let (dir, env) = rights
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .checked(&run)?;
    let input = match &run.input {
        Some(file) => Stdio::from(std::fs::File::open(file)?),
        None => Stdio::null(),
    };
    let mut child = tokio::process::Command::new(&run.program)
        .args(&run.args)
        .env_clear()
        .envs(env)
        .current_dir(dir)
        .stdin(input)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()?;
    let mut group = Group(child.id());
    let mut reading = tokio::spawn(said_of(child.stderr.take()));
    let Ok(status) = tokio::time::timeout(run.within, child.wait()).await else {
        reading.abort();
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "the run took too long",
        ));
    };
    let status = status?;
    // As git: the run ends with its program. A process that it left in the background is not
    // waited for, and keeps running.
    group.0 = None;
    let said = if let Ok(Ok(said)) = tokio::time::timeout(SAID_AFTER_END, &mut reading).await {
        said
    } else {
        reading.abort();
        String::new()
    };
    Ok(Ended {
        code: status.code(),
        said,
    })
}

/// The process group of a run while its program runs: each of its processes ends when the run
/// is dropped or takes longer than its time.
struct Group(Option<u32>);

impl Drop for Group {
    fn drop(&mut self) {
        let group = self
            .0
            .and_then(|id| i32::try_from(id).ok())
            .and_then(rustix::process::Pid::from_raw);
        if let Some(group) = group {
            let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
        }
    }
}

/// The first [`MOST_SAID`] bytes of the standard error; the rest is read and left.
async fn said_of(stderr: Option<tokio::process::ChildStderr>) -> String {
    let Some(mut stderr) = stderr else {
        return String::new();
    };
    let mut kept = Vec::new();
    let mut buffer = [0_u8; 8192];
    while let Ok(read) = stderr.read(&mut buffer).await {
        if read == 0 {
            break;
        }
        let room = MOST_SAID.saturating_sub(kept.len());
        kept.extend_from_slice(&buffer[..read.min(room)]);
    }
    String::from_utf8_lossy(&kept).into_owned()
}

impl Runner {
    /// The rights of the launcher as the threads and the login are now: each thread that runs,
    /// with its mode and its folder.
    pub(super) fn rights_now(&self) {
        let threads = self
            .threads
            .values()
            .filter(|thread| thread.row.state != RunState::Stopped)
            .map(|thread| (thread.row.id, (thread.row.mode, thread.row.folder.clone())))
            .collect();
        let mut rights = self.rights.lock().unwrap_or_else(PoisonError::into_inner);
        rights.env.clone_from(&self.env);
        rights.threads = threads;
    }
}
