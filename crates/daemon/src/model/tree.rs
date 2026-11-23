//! The process tree of a server (Section 9, R-331): the processes that the loopback check reads
//! and that a stop ends. Before each signal the daemon checks the id and the start time of the
//! process, so a process id that the system gave to a different program is never signalled.

#[cfg(target_os = "linux")]
pub mod delegation;
#[cfg(target_os = "linux")]
pub mod linux;
pub mod mounts;
pub mod walk;

use std::io;
use std::path::{Path, PathBuf};

use std::os::fd::OwnedFd;

use rustix::fs::{Mode, OFlags};
use rustix::process::{Pid, Signal};
use serde::{Deserialize, Serialize};

pub const RECORD: &str = "tree.json";
/// The launcher writes its exit code here before it exits at a failed step, so an exit code of
/// the runtime is never read as one of the launcher.
pub const LAUNCH_FAILURE: &str = "launch-failure";

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Identity {
    pub pid: i32,
    pub start: String,
}

pub const RECORD_BYTES: usize = 64 << 10;
pub const LAUNCH_FAILURE_BYTES: usize = 4 << 10;

fn open_folder(path: &Path) -> Option<OwnedFd> {
    let flags = OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::RDONLY | OFlags::DIRECTORY;
    rustix::fs::open(path, flags, Mode::empty()).ok()
}

/// `O_NOFOLLOW` checks the last part of the folder and the file only; the parent folders are
/// trusted because the daemon made them with [`super::folders::private_below`].
#[must_use]
pub fn read_capped(path: &Path, cap: usize) -> Option<Vec<u8>> {
    read_capped_at(&open_folder(path.parent()?)?, path.file_name()?, cap)
}

fn read_capped_at(dir: &OwnedFd, name: &std::ffi::OsStr, cap: usize) -> Option<Vec<u8>> {
    use rustix::fs::FileType;
    use std::io::Read as _;
    let flags = OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::RDONLY | OFlags::NONBLOCK;
    let fd = rustix::fs::openat(dir, name, flags, Mode::empty()).ok()?;
    let stat = rustix::fs::fstat(&fd).ok()?;
    let size = usize::try_from(stat.st_size).ok()?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile || size > cap {
        return None;
    }
    let mut bytes = Vec::new();
    std::fs::File::from(fd)
        .take(u64::try_from(cap).ok()? + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= cap).then_some(bytes)
}

/// A new temporary file and a rename, so a link or a FIFO at `name` is replaced, never
/// followed, and no other file is truncated through a hard link.
fn write_replacing(dir: &OwnedFd, name: &std::ffi::OsStr, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write as _;
    let mut temporary = std::ffi::OsString::from(".");
    temporary.push(name);
    temporary.push(format!(".{}", std::process::id()));
    let flags = OFlags::CREATE
        | OFlags::EXCL
        | OFlags::NOFOLLOW
        | OFlags::NONBLOCK
        | OFlags::WRONLY
        | OFlags::CLOEXEC;
    let _ = rustix::fs::unlinkat(dir, temporary.as_os_str(), rustix::fs::AtFlags::empty());
    let fd = rustix::fs::openat(
        dir,
        temporary.as_os_str(),
        flags,
        Mode::from_raw_mode(0o600),
    )?;
    std::fs::File::from(fd).write_all(bytes)?;
    rustix::fs::renameat(dir, temporary.as_os_str(), dir, name)?;
    Ok(())
}

#[must_use]
pub fn start_of(pid: i32) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        linux::start_of(pid)
    }
    #[cfg(not(target_os = "linux"))]
    {
        walk::start_of(pid)
    }
}

#[must_use]
pub fn identity(pid: i32) -> Option<Identity> {
    start_of(pid).map(|start| Identity { pid, start })
}

#[must_use]
pub fn signal(identity: &Identity, signal: Signal) -> bool {
    let Some(pid) = Pid::from_raw(identity.pid) else {
        return false;
    };
    if start_of(identity.pid).as_deref() != Some(identity.start.as_str()) {
        return false;
    }
    rustix::process::kill_process(pid, signal).is_ok()
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub known: Vec<Identity>,
    pub cgroup: Option<PathBuf>,
}

impl Record {
    /// A failure is not fatal: the record only helps the sweep after a crash.
    pub fn save(&self, path: &Path) {
        let (Some(folder), Some(name)) = (path.parent(), path.file_name()) else {
            return;
        };
        let (Ok(text), Some(dir)) = (serde_json::to_vec(self), open_folder(folder)) else {
            return;
        };
        let _ = write_replacing(&dir, name, &text);
    }

    fn load_at(dir: &OwnedFd) -> Option<Self> {
        serde_json::from_slice(&read_capped_at(dir, RECORD.as_ref(), RECORD_BYTES)?).ok()
    }

    /// It waits and runs `ps` (macOS), so async code calls it on a blocking thread.
    pub fn end_all(&self) {
        // A record can be written by the runtime when the sandbox is off; only a cgroup of a
        // server of this daemon is ended (S5).
        #[cfg(target_os = "linux")]
        if let Some(cgroup) = self
            .cgroup
            .as_deref()
            .filter(|path| linux::is_server_cgroup(path))
        {
            linux::end_cgroup(cgroup);
        }
        for identity in &self.known {
            let _gone_or_sent = signal(identity, Signal::KILL);
        }
    }
}

/// The daemon starts no server by itself (MW-P5), so each record is an orphan of an earlier run.
///
/// # Errors
/// An I/O error of the folder; a record that does not read is removed with no signal.
pub fn sweep(servers: &Path) -> io::Result<usize> {
    let entries = match std::fs::read_dir(servers) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    let mut swept = 0;
    for entry in entries.filter_map(Result::ok) {
        // One handle with no link followed for the read and the removal, so a folder that is a
        // link is neither read nor emptied.
        let Some(dir) = open_folder(&entry.path()) else {
            continue;
        };
        if let Some(found) = Record::load_at(&dir) {
            found.end_all();
            swept += 1;
        }
        let _ = rustix::fs::unlinkat(&dir, RECORD, rustix::fs::AtFlags::empty());
    }
    #[cfg(target_os = "linux")]
    linux::sweep_cgroups();
    Ok(swept)
}
