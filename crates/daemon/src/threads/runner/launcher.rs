//! The launcher of the runner: it starts one program for a thread with the rights of that
//! thread, for the Git engine ([`AsThread`]), so a hook of a repository never runs with the
//! rights of the runner (THR-050).
//!
//! Two runs. A read of the repository ([`read`], TH116) runs for each thread of this runner, in
//! the confinement of that thread; the launcher checks its whole shape itself. Each other run
//! is a hook, and runs only for a thread in Full access, so with the rights that its agent has
//! already: the engine runs no hook for a confined thread
//! ([`crate::threads::git::run::hooks::of`]), and this launcher refuses it again, so no change
//! of the engine can widen it. A hook gets the environment of the login that the agents get,
//! with no name of Anima (THR-040), the mark of its thread (THR-049) and only the names of the
//! Git engine that [`ADDED`] lists. A run has a process group of its own, which ends at its time
//! limit and when the run is dropped.
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

use read::Confinement;

use super::Runner;
use crate::threads::git::place::records::held_by_any;
use crate::threads::git::place::shared::pack::is_read;
use crate::threads::git::run::hooks::{AsThread, Ended, Kind, ThreadRun};
use crate::threads::marker;

pub mod read;
mod seat;

pub use seat::Seat;

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
    /// Each thread that runs, or whose worktree the runner makes: its folder and its rights.
    pub(super) threads: BTreeMap<Uuid, Seat>,
    /// The `git` of the Git engine: the one program of a read.
    pub(super) git: Option<PathBuf>,
    /// The records folder of the Git engine, where the input of a pack is.
    pub(super) records: Option<PathBuf>,
    /// The copy of the program that started each thread of this runner.
    pub(super) copies: BTreeMap<Uuid, PathBuf>,
    /// The threads whose worktree the runner makes before they start: known from that moment.
    pub(super) coming: BTreeMap<Uuid, Seat>,
    /// The home of Codex for the reads of confined Codex threads ([`read::codex_home`]).
    pub(super) codex_home: Option<PathBuf>,
}

/// A run after its checks: what the launcher starts.
struct Checked {
    program: PathBuf,
    args: Vec<OsString>,
    dir: PathBuf,
    env: Vec<(OsString, OsString)>,
    input: Option<std::fs::File>,
}

/// The rights, shared by the runner and each clone of its launcher.
pub type SharedRights = Arc<Mutex<Rights>>;

impl Rights {
    /// Whether the sandbox of a thread that runs lets its agent write `records`: one of its
    /// roots holds them ([`crate::threads::git::Engine::hold`], THR-050). A thread in Full access
    /// has no sandbox and no roots: this check does not cover it (operator's ruling of
    /// 2026-11-23). A thread whose worktree is being made has no agent yet.
    #[must_use]
    pub fn hold(&self, records: &Path) -> bool {
        self.threads
            .iter()
            .filter(|(id, _)| !self.coming.contains_key(id))
            .any(|(_, seat)| held_by_any(records, &seat.roots))
    }

    /// What `run` starts, after the checks of its rights: a read for each thread, a hook only
    /// for a thread in Full access.
    fn checked(&self, run: &ThreadRun) -> io::Result<Checked> {
        let Some(seat) = self.threads.get(&run.thread) else {
            return Err(denied("the thread does not run in this runner"));
        };
        let dir = std::fs::canonicalize(&run.dir)?;
        if !dir.starts_with(std::fs::canonicalize(&seat.folder)?) {
            return Err(denied(
                "the folder of the run is not in the folder of the thread",
            ));
        }
        let (mark, key) = marker::entry(&run.thread.to_string());
        let mark = (OsString::from(mark), OsString::from(key));
        if run.kind == Kind::Read {
            return self.read(run, seat, (dir, mark));
        }
        if seat.confinement.mode != Mode::FullAccess {
            return Err(denied(
                "a thread that is not in Full access runs no program but a read",
            ));
        }
        if !run.program.is_absolute() {
            return Err(denied("the program is not named by its full path"));
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
        let env = login.chain(added).chain([mark]).collect();
        let input = run.input.as_ref().map(std::fs::File::open).transpose()?;
        Ok(Checked {
            program: run.program.clone(),
            args: run.args.clone(),
            dir,
            env,
            input,
        })
    }

    /// A read of the repository ([`read`]), for each thread in its confinement.
    fn read(
        &self,
        run: &ThreadRun,
        seat: &Seat,
        (dir, mark): (PathBuf, (OsString, OsString)),
    ) -> io::Result<Checked> {
        let (Some(git), Some(records)) = (&self.git, &self.records) else {
            return Err(denied(
                "the runner gave the Git engine no git or no records",
            ));
        };
        if !is_read(run, git, records) {
            return Err(denied("the run is not a read of the Git engine"));
        }
        let git = (run.program.as_path(), dir.as_path(), run.args.as_slice());
        let (program, args, sandboxed) = read::confined(run.thread, &seat.confinement, git)?;
        let home = if sandboxed {
            let Some(home) = &self.codex_home else {
                return Err(denied("the runner has no home of Codex for reads"));
            };
            read::codex_home(home)?;
            Some((OsString::from("CODEX_HOME"), home.clone().into_os_string()))
        } else {
            None
        };
        let input = run.input.as_deref().map(read::listed).transpose()?;
        let login = self
            .env
            .iter()
            .filter(|(name, _)| read::login_env(name.as_ref()) && name != "CODEX_HOME")
            .map(|(name, value)| (OsString::from(name), OsString::from(value)));
        // A read writes only the `TMPDIR` of its thread, never the one of the login (THR-050).
        let tmp = super::start::private_tmp::NAMES
            .map(|name| (OsString::from(name), seat.tmp.clone().into_os_string()));
        let own = run
            .env
            .iter()
            .map(|(name, value)| (OsString::from(name), value.clone()));
        // No transport at all, whatever a configuration that the thread can write says: a
        // `git` older than 2.44 does not read `GIT_NO_LAZY_FETCH`, and a `protocol.<name>.allow`
        // of the repository wins over `protocol.allow`. This list wins over both, and names no
        // protocol.
        let closed = (OsString::from("GIT_ALLOW_PROTOCOL"), OsString::from("none"));
        let env = login
            .chain(tmp)
            .chain(own)
            .chain(home)
            .chain([closed, mark])
            .collect();
        Ok(Checked {
            program,
            args,
            dir,
            env,
            input,
        })
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
    let checked = rights
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .checked(&run)?;
    let input = checked.input.map_or_else(Stdio::null, Stdio::from);
    let mut child = tokio::process::Command::new(&checked.program)
        .args(&checked.args)
        .env_clear()
        .envs(checked.env)
        .current_dir(checked.dir)
        .stdin(input)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .process_group(0)
        .kill_on_drop(true)
        .spawn()?;
    let mut group = Group(child.id());
    let mut kept = Vec::new();
    let waited = {
        let reading = read_into(child.stderr.take(), &mut kept);
        tokio::pin!(reading);
        let ended = async {
            tokio::select! {
                status = child.wait() => {
                    // Reaped: its ID may now be of a different process, so it is never killed.
                    group.0 = None;
                    // As git: the run ends with its program; what it wrote just before its end
                    // is still read, for a moment.
                    let _ = tokio::time::timeout(SAID_AFTER_END, &mut reading).await;
                    status
                }
                () = &mut reading => {
                    let status = child.wait().await;
                    group.0 = None;
                    status
                }
            }
        };
        tokio::time::timeout(run.within, ended).await
    };
    let Ok(status) = waited else {
        return Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "the run took too long",
        ));
    };
    // A process that the program left in the background is not waited for, and keeps running.
    let status = status?;
    Ok(Ended {
        code: status.code(),
        said: String::from_utf8_lossy(&kept).into_owned(),
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

/// Reads the standard error into `kept` until its end, keeping the first [`MOST_SAID`] bytes.
async fn read_into(stderr: Option<tokio::process::ChildStderr>, kept: &mut Vec<u8>) {
    let Some(mut stderr) = stderr else {
        return;
    };
    let mut buffer = [0_u8; 8192];
    while let Ok(read) = stderr.read(&mut buffer).await {
        if read == 0 {
            break;
        }
        let room = MOST_SAID.saturating_sub(kept.len());
        kept.extend_from_slice(&buffer[..read.min(room)]);
    }
}

impl Runner {
    /// The rights again when a row changed: a mode, a stop.
    pub(super) fn rights_if(&self, row: bool) {
        if row {
            self.rights_now();
        }
    }

    /// The rights of the launcher as the threads and the login are now: each thread that runs,
    /// or whose worktree the runner makes, with its folder and its rights.
    pub(super) fn rights_now(&self) {
        self.rights_of_threads();
        self.git_hold();
    }

    /// Each thread that runs, or whose worktree the runner makes, with its folder and its rights.
    /// The `TMPDIR` of a thread that no longer runs goes; a new start makes it again.
    fn rights_of_threads(&self) {
        let keep = self.seats_now();
        super::start::private_tmp::sweep(&self.dir, |thread| keep.contains(&thread));
    }

    /// The `TMPDIR` of a running thread, made again when it is not there. When it cannot be
    /// made, its path as named: the hold counts a root that does not resolve as one that holds.
    fn tmp_of(&self, thread: Uuid) -> PathBuf {
        super::start::private_tmp::made(&self.dir, thread).unwrap_or_else(|error| {
            tracing::warn!(%error, %thread, "the temporary folder of a thread could not be made");
            super::start::private_tmp::folder(&self.dir, thread)
        })
    }

    /// Writes the seats of the threads that run or are coming, and gives their IDs.
    fn seats_now(&self) -> std::collections::BTreeSet<Uuid> {
        let daemon = self.dir.parent().unwrap_or(&self.dir);
        // The folders are made before the lock: the launcher does not wait on this I/O.
        let running: Vec<_> = self
            .threads
            .values()
            .filter(|thread| thread.row.state != RunState::Stopped)
            .map(|thread| (&thread.row, self.tmp_of(thread.row.id)))
            .collect();
        let mut rights = self.rights.lock().unwrap_or_else(PoisonError::into_inner);
        let live: BTreeMap<Uuid, Seat> = running
            .into_iter()
            .map(|(row, tmp)| {
                let confinement = Confinement {
                    mode: row.mode,
                    program: row.program,
                    codex: rights.copies.get(&row.id).cloned(),
                    kept: crate::threads::notice::held_anima(&row.folder, daemon),
                };
                (row.id, Seat::new(row.folder.clone(), confinement, tmp))
            })
            .collect();
        let rights = &mut *rights;
        // A start whose holder refused it is neither coming nor running any more.
        rights
            .coming
            .retain(|id, _| !self.threads.contains_key(id) && self.asks.making(*id));
        rights
            .copies
            .retain(|id, _| live.contains_key(id) || rights.coming.contains_key(id));
        rights.threads = live;
        rights
            .threads
            .extend(rights.coming.iter().map(|(id, seat)| (*id, seat.clone())));
        rights.env.clone_from(&self.env);
        rights.codex_home = Some(self.dir.join("codex-reads"));
        rights.threads.keys().copied().collect()
    }

    /// The copy of the program that started `thread`: a read of a confined Codex thread runs in
    /// the sandbox of that copy.
    pub(super) fn rights_copy(&self, thread: Uuid, copy: PathBuf) {
        let mut rights = self.rights.lock().unwrap_or_else(PoisonError::into_inner);
        rights.copies.insert(thread, copy);
    }
}
