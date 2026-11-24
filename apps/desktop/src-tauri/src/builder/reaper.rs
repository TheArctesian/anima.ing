//! On Linux this process is the child subreaper of its descendants: every live descendant with the
//! step's marker is killed and reaped after the step and on cancel (BLD-026, D-20). A process the
//! app did not start through a step is never marked, so it is never killed or waited for.
//! A process started without the marker, and on macOS (no subreaper) any that called `setsid`, escapes.

pub const MARKER: &str = "ANIMA_BUILD_STEP";

#[cfg_attr(
    not(target_os = "linux"),
    expect(dead_code, reason = "only the Linux subreaper reads the process table")
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Process {
    pub pid: i32,
    pub parent: i32,
    pub group: i32,
    pub session: i32,
    pub zombie: bool,
}

/// The command name may hold spaces and parentheses, so the fields are read after the last `)`.
#[cfg_attr(
    not(target_os = "linux"),
    expect(dead_code, reason = "only the Linux subreaper reads the process table")
)]
pub fn process_of(pid: i32, stat: &str) -> Option<Process> {
    let (_, fields) = stat.rsplit_once(')')?;
    let mut fields = fields.split_whitespace();
    let state = fields.next()?;
    let parent = fields.next()?.parse().ok()?;
    let group = fields.next()?.parse().ok()?;
    let session = fields.next()?.parse().ok()?;
    Some(Process {
        pid,
        parent,
        group,
        session,
        zombie: state == "Z" || state == "X",
    })
}

#[cfg_attr(
    not(target_os = "linux"),
    expect(dead_code, reason = "only the Linux subreaper reads the process table")
)]
pub fn descendants(root: i32, all: &[Process]) -> Vec<Process> {
    let mut found: Vec<Process> = Vec::new();
    let mut parents = vec![root];
    while let Some(parent) = parents.pop() {
        for process in all.iter().filter(|process| process.parent == parent) {
            if process.pid != root && !found.iter().any(|seen| seen.pid == process.pid) {
                found.push(*process);
                parents.push(process.pid);
            }
        }
    }
    found
}

#[cfg_attr(
    not(target_os = "linux"),
    expect(dead_code, reason = "only the Linux subreaper reads the process table")
)]
pub fn marked(environ: &[u8], value: &str) -> bool {
    let wanted = format!("{MARKER}={value}");
    environ
        .split(|byte| *byte == 0)
        .any(|entry| entry == wanted.as_bytes())
}

#[cfg(target_os = "linux")]
mod linux {
    use std::{collections::HashSet, fs, io::Read, sync::Once, thread, time::Duration};

    use nix::{
        sys::{
            signal::{Signal, kill},
            wait::{WaitPidFlag, waitpid},
        },
        unistd::Pid,
    };

    use super::{Process, descendants, marked, process_of};

    const ENVIRON_MAX_BYTES: u64 = 1 << 20;
    const ROUNDS: usize = 200;
    const PAUSE: Duration = Duration::from_millis(10);

    pub fn become_subreaper() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            let _ = nix::sys::prctl::set_child_subreaper(true);
        });
    }

    fn processes() -> Vec<Process> {
        let Ok(entries) = fs::read_dir("/proc") else {
            return Vec::new();
        };
        entries
            .filter_map(Result::ok)
            .filter_map(|entry| entry.file_name().to_str()?.parse::<i32>().ok())
            .filter_map(|pid| {
                process_of(pid, &fs::read_to_string(format!("/proc/{pid}/stat")).ok()?)
            })
            .collect()
    }

    fn carries(pid: i32, value: &str) -> bool {
        let mut environ = Vec::new();
        fs::File::open(format!("/proc/{pid}/environ"))
            .and_then(|file| file.take(ENVIRON_MAX_BYTES).read_to_end(&mut environ))
            .is_ok_and(|_| marked(&environ, value))
    }

    fn me() -> i32 {
        i32::try_from(std::process::id()).unwrap_or(i32::MAX)
    }

    /// The pidfd is opened before the marker is read, so the signal reaches the process that was
    /// read even if its ID was reused meanwhile. Without pidfd (Linux before 5.3), `kill`.
    fn kill_if_marked(pid: i32, value: &str) -> bool {
        let Some(id) = rustix::process::Pid::from_raw(pid) else {
            return false;
        };
        match rustix::process::pidfd_open(id, rustix::process::PidfdFlags::empty()) {
            Ok(pidfd) => {
                carries(pid, value)
                    && rustix::process::pidfd_send_signal(&pidfd, rustix::process::Signal::KILL)
                        .is_ok()
            }
            Err(rustix::io::Errno::NOSYS) => {
                carries(pid, value) && kill(Pid::from_raw(pid), Signal::SIGKILL).is_ok()
            }
            Err(_) => false,
        }
    }

    pub fn kill_marked(value: &str) -> Vec<i32> {
        let all = processes();
        descendants(me(), &all)
            .into_iter()
            .filter(|process| !process.zombie && kill_if_marked(process.pid, value))
            .map(|process| process.pid)
            .collect()
    }

    /// Waits only for processes seen with the marker or in the step's process `group`; a child the
    /// app
    /// waits for elsewhere, in this session, is never taken from it. Run it after the step's own
    /// process was waited for.
    pub fn sweep(value: &str, group: Option<i32>) {
        let mut killed: HashSet<i32> = HashSet::new();
        for _ in 0..ROUNDS {
            let live = kill_marked(value);
            killed.extend(&live);
            let me = me();
            let all = processes();
            let session = all
                .iter()
                .find(|process| process.pid == me)
                .map(|process| process.session);
            let step =
                |process: &Process| killed.contains(&process.pid) || Some(process.group) == group;
            let reapable = |process: &Process| {
                process.parent == me
                    && process.zombie
                    && (step(process) || Some(process.session) != session)
            };
            for process in all.iter().filter(|process| reapable(process)) {
                let _ = waitpid(Pid::from_raw(process.pid), Some(WaitPidFlag::WNOHANG));
            }
            let pending = all
                .iter()
                .any(|process| process.parent == me && !process.zombie && step(process));
            if live.is_empty() && !pending {
                return;
            }
            thread::sleep(PAUSE);
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::{become_subreaper, kill_marked, sweep};

#[cfg(not(target_os = "linux"))]
pub fn become_subreaper() {}

#[cfg(not(target_os = "linux"))]
pub fn kill_marked(_value: &str) -> Vec<i32> {
    Vec::new()
}

#[cfg(not(target_os = "linux"))]
pub fn sweep(_value: &str, _group: Option<i32>) {}
