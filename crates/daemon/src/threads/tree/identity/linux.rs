use std::os::fd::{AsFd, AsRawFd as _, OwnedFd};
use std::sync::Arc;

use nix_peer::sys::socket::{getsockopt, sockopt::PeerPidfd};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::fs::{fstat, fstatfs, statfs};
use rustix::process::{Pid, PidfdFlags, pidfd_open};

use super::Identity;

const PID_FS_MAGIC: u32 = 0x5049_4446;
const PROC_FS_MAGIC: u32 = 0x9fa0;
static PROC_NAMESPACE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

#[derive(Clone, Debug)]
pub struct Process {
    fd: Arc<OwnedFd>,
    identity: Identity,
}

impl Process {
    pub fn peer(socket: &impl AsFd, pid: Option<i32>) -> Option<Self> {
        let expected = u32::try_from(pid?).ok()?;
        let peer = Self::from_fd(getsockopt(socket, PeerPidfd).ok()?)?;
        (peer.identity.pid == expected).then_some(peer)
    }

    pub fn open(pid: u32) -> Option<Self> {
        let asked = Pid::from_raw(i32::try_from(pid).ok()?)?;
        let process = Self::from_fd(pidfd_open(asked, PidfdFlags::empty()).ok()?)?;
        (process.identity.pid == pid).then_some(process)
    }

    fn from_fd(fd: OwnedFd) -> Option<Self> {
        if !*PROC_NAMESPACE.get_or_init(proc_matches_namespace) {
            return None;
        }
        let statfs = fstatfs(&fd).ok()?;
        if u32::try_from(statfs.f_type).ok()? != PID_FS_MAGIC {
            return None;
        }
        let stat = fstat(&fd).ok()?;
        let pid = fd_pid(&fd)?;
        let process = Self {
            fd: Arc::new(fd),
            identity: Identity {
                pid,
                device: stat.st_dev,
                inode: stat.st_ino,
            },
        };
        process.live().then_some(process)
    }

    pub const fn identity(&self) -> Identity {
        self.identity
    }

    pub fn live(&self) -> bool {
        let mut fd = [PollFd::new(self.fd.as_ref(), PollFlags::IN)];
        matches!(poll(&mut fd, Some(&Timespec::default())), Ok(0))
    }

    pub fn parent(&self) -> Option<Self> {
        let pid = parent_pid(self.identity.pid)?;
        let parent = Self::open(pid)?;
        // The first PPid may have disappeared while the parent handle was opened.
        (parent_pid(self.identity.pid)? == pid && self.live() && parent.live()).then_some(parent)
    }
}

fn fd_pid(fd: &OwnedFd) -> Option<u32> {
    let path = format!("/proc/self/fdinfo/{}", fd.as_raw_fd());
    let text = std::fs::read_to_string(path).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("Pid:"))?
        .trim()
        .parse()
        .ok()
        .filter(|pid| *pid > 0)
}

fn parent_pid(pid: u32) -> Option<u32> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (name, rest) = stat.rsplit_once(") ")?;
    let read_pid = name.split_once(" (")?.0.parse::<u32>().ok()?;
    (read_pid == pid).then_some(())?;
    rest.split_whitespace().nth(1)?.parse().ok()
}

fn proc_matches_namespace() -> bool {
    let status = std::fs::read_to_string("/proc/self/status");
    if !statfs("/proc").is_ok_and(|fs| u32::try_from(fs.f_type) == Ok(PROC_FS_MAGIC)) {
        return false;
    }
    status.ok().is_some_and(|status| {
        let Some(ids) = status.lines().find_map(|line| line.strip_prefix("NSpid:")) else {
            return false;
        };
        let mut ids = ids.split_whitespace();
        ids.next().and_then(|id| id.parse().ok()) == Some(std::process::id())
            && ids.next().is_none()
    })
}

#[cfg(test)]
mod tests;
