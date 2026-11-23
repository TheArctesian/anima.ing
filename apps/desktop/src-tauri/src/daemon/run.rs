//! Runs the installed daemon once (MET-019, D-10 d): no shell, only the variables it needs, the
//! code on standard input only, and its whole process group killed at the deadline or the app's
//! exit. It logs nothing (LOG-007).

use std::{
    ffi::OsString,
    io::{ErrorKind, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

/// The user session bus that `systemctl --user` needs.
#[cfg(unix)]
const PASSED_ON: [&str; 2] = ["XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS"];

#[cfg(windows)]
const PASSED_ON: [&str; 7] = [
    "SystemRoot",
    "WINDIR",
    "USERPROFILE",
    "LOCALAPPDATA",
    "APPDATA",
    "TEMP",
    "TMP",
];

const POLL: Duration = Duration::from_millis(20);

/// `ETXTBSY`, the same number on Linux and macOS.
const TEXT_BUSY: i32 = 26;
const BUSY_TRIES: u32 = 5;
const BUSY_PAUSE: Duration = Duration::from_millis(50);

pub struct Run<'a> {
    pub program: &'a Path,
    pub args: &'a [&'a str],
    pub input: Option<&'a [u8]>,
    pub env: &'a [(&'static str, OsString)],
    pub deadline: Duration,
    pub stop: &'a AtomicBool,
}

/// Fixed text only: no OS text, no argument and no input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RunError {
    #[error("the daemon could not start")]
    CouldNotStart,
    #[error("the daemon could not be given its input")]
    Input,
    #[error("the daemon exited with a failure")]
    Failed,
    #[error("the daemon ran past its deadline")]
    TimedOut,
    #[error("the app stopped the daemon as it exited")]
    Stopped,
    #[error("the daemon's exit status could not be read")]
    Unreadable,
}

/// `enrol --hub <origin> -`: the code comes on standard input, never on the command line.
pub fn enrol_args(origin: &str) -> [&str; 4] {
    ["enrol", "--hub", origin, "-"]
}

/// `install`: the user unit or `LaunchAgent`; never `--system`, which needs root.
pub const INSTALL_ARGS: [&str; 1] = ["install"];

/// `XDG_CONFIG_HOME` stays unset so the daemon's home is `$HOME/.config/anima-daemon` (T6a), the
/// only place its `install` accepts a token.
pub fn daemon_env(
    home: &Path,
    lookup: impl Fn(&str) -> Option<OsString>,
) -> Vec<(&'static str, OsString)> {
    let mut env = vec![("HOME", home.as_os_str().to_owned())];
    for name in PASSED_ON {
        if let Some(value) = lookup(name).filter(|value| !value.is_empty()) {
            env.push((name, value));
        }
    }
    env
}

/// # Errors
/// [`RunError`] when the program cannot start, take its input or exit in time, or is stopped.
pub fn run(daemon_run: &Run<'_>) -> Result<(), RunError> {
    finish(daemon_run, false).map(drop)
}

/// The pipe is read without waiting, so a process it started that keeps the pipe open cannot
/// hold the app.
///
/// # Errors
/// [`RunError`] as [`run`]; [`RunError::Unreadable`] when the output cannot be read.
#[cfg(unix)]
pub fn output(daemon_run: &Run<'_>, limit: u64) -> Result<Vec<u8>, RunError> {
    use std::io::Read as _;
    let mut child = finish(daemon_run, true)?;
    let stdout = child.stdout.take().ok_or(RunError::Unreadable)?;
    let flags = nix::fcntl::fcntl(&stdout, nix::fcntl::FcntlArg::F_GETFL)
        .map_err(|_| RunError::Unreadable)?;
    let flags = nix::fcntl::OFlag::from_bits_truncate(flags) | nix::fcntl::OFlag::O_NONBLOCK;
    nix::fcntl::fcntl(&stdout, nix::fcntl::FcntlArg::F_SETFL(flags))
        .map_err(|_| RunError::Unreadable)?;
    let mut out = Vec::new();
    match stdout.take(limit).read_to_end(&mut out) {
        Ok(_) => Ok(out),
        Err(error) if error.kind() == ErrorKind::WouldBlock => Ok(out),
        Err(_) => Err(RunError::Unreadable),
    }
}

fn finish(daemon_run: &Run<'_>, capture: bool) -> Result<Child, RunError> {
    if daemon_run.stop.load(Ordering::Acquire) {
        return Err(RunError::Stopped);
    }
    let mut child = start(daemon_run, capture)?;
    #[cfg(windows)]
    let Ok(_job) = anima_windows::job::Job::start(child.id()) else {
        reap(child);
        return Err(RunError::CouldNotStart);
    };
    if let Err(error) = hand_over(&mut child, daemon_run.input) {
        reap(child);
        return Err(error);
    }
    let end = Instant::now() + daemon_run.deadline;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(child),
            Ok(Some(_)) => return Err(RunError::Failed),
            Ok(None) if daemon_run.stop.load(Ordering::Acquire) => {
                reap(child);
                return Err(RunError::Stopped);
            }
            Ok(None) if Instant::now() < end => thread::sleep(POLL),
            Ok(None) => {
                reap(child);
                return Err(RunError::TimedOut);
            }
            Err(_) => {
                reap(child);
                return Err(RunError::Unreadable);
            }
        }
    }
}

/// A just-written binary is busy while another thread forks with the file open; that start is
/// tried again.
fn start(daemon_run: &Run<'_>, capture: bool) -> Result<Child, RunError> {
    let mut tries = 0;
    loop {
        let mut command = Command::new(daemon_run.program);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            command.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            command.creation_flags(anima_windows::job::SUSPENDED);
        }
        let started = command
            .args(daemon_run.args)
            .env_clear()
            .envs(daemon_run.env.iter().map(|(name, value)| (name, value)))
            .current_dir("/")
            .stdin(if daemon_run.input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(if capture {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stderr(Stdio::null())
            .spawn();
        match started {
            Err(error) if error.raw_os_error() == Some(TEXT_BUSY) && tries < BUSY_TRIES => {
                tries += 1;
                thread::sleep(BUSY_PAUSE);
            }
            started => return started.map_err(|_| RunError::CouldNotStart),
        }
    }
}

/// A broken pipe is not an error: the child's exit status tells what happened.
fn hand_over(child: &mut Child, input: Option<&[u8]>) -> Result<(), RunError> {
    let Some(input) = input else {
        return Ok(());
    };
    let mut stdin = child.stdin.take().ok_or(RunError::Input)?;
    match stdin.write_all(input) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::BrokenPipe => Ok(()),
        Err(_) => Err(RunError::Input),
    }
}

/// The child is reaped only after the kill, so its pid still names its own group and no other.
fn reap(mut child: Child) {
    #[cfg(unix)]
    if let Ok(group) = i32::try_from(child.id()) {
        // A group that is gone is not an error: the wait below still reaps the child.
        let _ = nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(group),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
    let _ = child.kill();
    let _ = child.wait();
}
