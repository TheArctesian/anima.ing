//! A server process in its tree, with an empty environment and its output in the ring (Sections 2.1 and 9).

#[cfg(target_os = "linux")]
mod handshake;
#[cfg(target_os = "linux")]
mod join;
mod starter;

use std::io;
use std::net::TcpListener;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::time::Duration;

use daemon_core::model_work::cause::{Cause, Containment};
use rustix::process::Signal;
use tokio::process::Child;
use zeroize::{Zeroize as _, Zeroizing};

use super::ring::Ring;
use super::tree::{self, Identity, Record};

pub const PORTS: RangeInclusive<u16> = 41_000..=41_999;

/// Each word comes from the code of the daemon; the environment holds the API key.
pub struct Launch {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub environment: Vec<(String, String)>,
    pub state: PathBuf,
    pub temporary: PathBuf,
    pub ring: PathBuf,
    pub key: Zeroizing<String>,
    pub place: PathBuf,
    pub server: uuid::Uuid,
    pub launcher: PathBuf,
}

impl Drop for Launch {
    fn drop(&mut self) {
        for (_, value) in &mut self.environment {
            value.zeroize();
        }
    }
}

/// A running server; its drop ends the whole tree, so no process outlives the daemon's record.
pub struct Running {
    pub child: Child,
    pub leader: Identity,
    pub known: Vec<Identity>,
    pub cgroup: Option<PathBuf>,
    record: PathBuf,
    pub key: Zeroizing<String>,
    ended: bool,
    #[cfg(target_os = "linux")]
    startup: Option<tokio::net::UnixStream>,
}

impl Drop for Running {
    fn drop(&mut self) {
        if !self.ended {
            self.end_now();
        }
    }
}

/// The parts that end the tree, without the child handle, so the end can run on a blocking thread.
#[derive(Clone)]
struct Tree {
    leader: Identity,
    known: Vec<Identity>,
    cgroup: Option<PathBuf>,
    record: PathBuf,
}

impl Tree {
    fn signal_all(&self, signal: Signal) {
        for identity in self.known.iter().chain([&self.leader]) {
            let _gone_or_sent = tree::signal(identity, signal);
        }
    }

    /// On macOS the walk is read again first, so a child made after the last poll is ended too.
    #[cfg_attr(
        target_os = "linux",
        expect(unused_mut, reason = "only the macOS walk adds to known")
    )]
    fn end(mut self) {
        #[cfg(not(target_os = "linux"))]
        if let Some(table) = super::tree::walk::table() {
            let roots: Vec<Identity> = self.known.iter().chain([&self.leader]).cloned().collect();
            for found in super::tree::walk::descendants(&table, &roots) {
                if !self.known.contains(&found) {
                    self.known.push(found);
                }
            }
        }
        let record = Record {
            known: self.known.clone(),
            cgroup: self.cgroup.clone(),
        };
        record.end_all();
        self.signal_all(Signal::KILL);
        let _ = std::fs::remove_file(&self.record);
    }
}

impl Running {
    fn record(&self) -> Record {
        Record {
            known: self.known.clone(),
            cgroup: self.cgroup.clone(),
        }
    }

    /// The launcher step that failed, or `runtime_not_signed` (the model launcher, code 125); an
    /// exit code alone is not taken without the launcher's failure file.
    #[must_use]
    pub fn launch_failure(&self, status: std::process::ExitStatus) -> Option<Cause> {
        let exit = status.code()?;
        let cause = if exit == Containment::NOT_SIGNED_EXIT_CODE {
            Cause::RuntimeNotSigned
        } else {
            Cause::ContainmentFailed(Containment::of_exit_code(exit)?)
        };
        let path = self.record.with_file_name(tree::LAUNCH_FAILURE);
        let written = tree::read_capped(&path, tree::LAUNCH_FAILURE_BYTES)?;
        let code: Option<i32> = std::str::from_utf8(&written).ok()?.trim().parse().ok();
        (code == Some(exit)).then_some(cause)
    }

    fn tree(&self) -> Tree {
        Tree {
            leader: self.leader.clone(),
            known: self.known.clone(),
            cgroup: self.cgroup.clone(),
            record: self.record.clone(),
        }
    }

    /// # Errors
    /// Fails closed when the tree cannot be read or the running main process has left it.
    pub async fn poll(&mut self) -> io::Result<Vec<Identity>> {
        #[cfg(target_os = "linux")]
        if let Some(startup) = self.startup.take() {
            handshake::wait(startup).await?;
        }
        #[cfg(target_os = "linux")]
        let members = {
            let cgroup = self
                .cgroup
                .clone()
                .ok_or_else(|| io::Error::other("no cgroup"))?;
            tokio::task::spawn_blocking(move || super::tree::linux::members(&cgroup))
                .await
                .map_err(io::Error::other)??
        };
        #[cfg(not(target_os = "linux"))]
        let members = {
            let roots: Vec<Identity> = self.known.iter().chain([&self.leader]).cloned().collect();
            let read = tokio::task::spawn_blocking(super::tree::walk::table);
            let table = tokio::time::timeout(Duration::from_secs(5), read)
                .await
                .map_err(|_| io::Error::other("ps did not end in 5 seconds"))?
                .map_err(io::Error::other)?
                .ok_or_else(|| io::Error::other("ps did not read"))?;
            super::tree::walk::descendants(&table, &roots)
        };
        let running = matches!(self.child.try_wait(), Ok(None));
        if running && members.is_empty() {
            return Err(io::Error::other("the main process runs outside its tree"));
        }
        // A delegated cgroup lets a runtime move itself out; a running leader outside it has left (T1).
        #[cfg(target_os = "linux")]
        if running && !members.contains(&self.leader) {
            return Err(io::Error::other(
                "the main process left the cgroup of its server",
            ));
        }
        self.known.clone_from(&members);
        self.record().save(&self.record);
        Ok(members)
    }

    fn end_now(&mut self) {
        self.tree().end();
    }

    async fn end_off_thread(&self) {
        let tree = self.tree();
        let _ = tokio::task::spawn_blocking(move || tree.end()).await;
    }
}

/// The listener check after the start confirms that the server, not another program, holds the port.
#[must_use]
pub fn free_port(range: RangeInclusive<u16>) -> Option<u16> {
    range
        .into_iter()
        .find(|port| TcpListener::bind(("127.0.0.1", *port)).is_ok())
}

/// # Errors
/// An I/O error of the folder.
pub fn fresh_temporary(state: &Path, path: &Path) -> io::Result<()> {
    let relative = path
        .strip_prefix(state)
        .map_err(|_| io::Error::other("the temporary folder is not below the state folder"))?;
    let parent = relative.parent().unwrap_or(Path::new(""));
    // The parents are checked first, so the removal below follows no link of a parent.
    super::folders::private_below(state, parent)?;
    match std::fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    super::folders::private_below(state, relative).map(drop)
}

/// Call it from a long-lived runtime worker, not `spawn_blocking`: the launcher's parent-death
/// signal fires when the thread that started it ends ([`super::tree::linux::launch`]).
///
/// # Errors
/// `containment_failed` when the cgroup cannot be made, the model launcher is missing or
/// outdated, or the launcher is not in the cgroup after 5 seconds (Linux); `runtime_not_signed`
/// where the model launcher is used and the program is not on the kept list; `start_failed` for
/// an error of the folders, the ring or the start.
#[cfg_attr(
    not(target_os = "linux"),
    expect(
        clippy::unused_async,
        reason = "only the wait for the cgroup join awaits, and it is Linux only"
    )
)]
pub async fn spawn(launch: &Launch) -> Result<Running, Cause> {
    fresh_temporary(&launch.state, &launch.temporary).map_err(|_| Cause::StartFailed)?;
    match std::fs::remove_file(launch.place.join(tree::LAUNCH_FAILURE)) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Err(Cause::StartFailed),
    }
    let ring = Ring::create(&launch.ring).map_err(|_| Cause::StartFailed)?;
    let starter = starter::contain(launch).await?;
    let mut command = starter::command(launch, &starter);
    let cgroup = starter.cgroup();
    #[cfg(target_os = "linux")]
    let startup = Some(handshake::attach(&mut command).map_err(|_| Cause::StartFailed)?);
    let mut child = command.spawn().map_err(|_| Cause::StartFailed)?;
    drop(command);
    let leader = child
        .id()
        .and_then(|id| i32::try_from(id).ok())
        .and_then(tree::identity)
        .ok_or(Cause::StartFailed)?;
    if let Some(output) = child.stdout.take() {
        ring.follow(output);
    }
    if let Some(output) = child.stderr.take() {
        ring.follow(output);
    }
    let running = Running {
        child,
        known: vec![leader.clone()],
        leader,
        cgroup,
        record: launch.place.join(tree::RECORD),
        key: launch.key.clone(),
        ended: false,
        #[cfg(target_os = "linux")]
        startup,
    };
    running.record().save(&running.record);
    #[cfg(target_os = "linux")]
    let running = join::joined(running).await?;
    Ok(running)
}

pub async fn kill(running: &mut Running) {
    let _ = running.poll().await;
    running.end_off_thread().await;
    let _ = running.child.wait().await;
    running.ended = true;
}

pub async fn stop(running: &mut Running) {
    let _ = running.poll().await;
    let tree = running.tree();
    let _ = tokio::task::spawn_blocking(move || tree.signal_all(Signal::TERM)).await;
    let _ = tokio::time::timeout(Duration::from_secs(10), running.child.wait()).await;
    kill(running).await;
}
