//! One fixed command that `install` runs: an absolute program, no shell, no input and a deadline (MET-019, D-10).

use std::io;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub const STEP_DEADLINE: Duration = Duration::from_secs(30);

/// The only search path a command gets: the caller's own environment is not passed on.
#[cfg(unix)]
const PATH: &str = "/usr/sbin:/usr/bin:/sbin:/bin";

const POLL: Duration = Duration::from_millis(20);

#[derive(Debug, PartialEq, Eq)]
pub struct Step {
    pub program: &'static str,
    pub args: Vec<String>,
    /// A failure is expected and fine (`launchctl bootout` of a daemon that is not loaded).
    pub may_fail: bool,
    /// Not starting, or running past its deadline, is no install error either (R-126).
    pub may_not_run: bool,
    pub env: Vec<(&'static str, String)>,
    /// `launchctl bootstrap` right after `bootout` answers EIO for some seconds, so it gets several tries.
    pub retries: &'static [Duration],
}

impl Step {
    pub(super) fn new(program: &'static str, args: &[&str]) -> Self {
        Self {
            program,
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            may_fail: false,
            may_not_run: false,
            env: Vec::new(),
            retries: &[],
        }
    }

    /// The step with `args` in place of its arguments.
    #[cfg(target_os = "linux")]
    pub(super) fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }
}

/// `Ok(false)` is a failure exit.
///
/// # Errors
/// The I/O error when the program cannot start or be waited on, or [`io::ErrorKind::TimedOut`] past `deadline`.
pub fn run_step(step: &Step, deadline: Duration) -> io::Result<bool> {
    let mut ok = run_once(step, deadline)?;
    for pause in step.retries {
        if ok {
            break;
        }
        thread::sleep(*pause);
        ok = run_once(step, deadline)?;
    }
    Ok(ok)
}

fn run_once(step: &Step, deadline: Duration) -> io::Result<bool> {
    #[cfg(unix)]
    let program = std::path::PathBuf::from(step.program);
    #[cfg(windows)]
    let program = std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or(io::ErrorKind::NotFound)?
        .join("System32")
        .join(step.program);
    let mut command = Command::new(program);
    command.env_clear();
    #[cfg(unix)]
    command.env("PATH", PATH);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(0x0800_0000);
        for name in [
            "SystemRoot",
            "WINDIR",
            "USERPROFILE",
            "LOCALAPPDATA",
            "APPDATA",
            "TEMP",
            "TMP",
        ] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
    }
    let mut child = command
        .args(&step.args)
        .envs(step.env.iter().map(|(name, value)| (name, value)))
        .stdin(Stdio::null())
        .spawn()?;
    let end = Instant::now() + deadline;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status.success());
        }
        let now = Instant::now();
        if now >= end {
            // Killed through its own handle, then reaped, so no other process is ever touched.
            let killed = child.kill();
            child.wait()?;
            killed?;
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("did not finish within {} s", deadline.as_secs_f32()),
            ));
        }
        thread::sleep(POLL.min(end - now));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    const SHORT: Duration = Duration::from_millis(10);

    #[test]
    fn met_019_a_step_that_runs_past_its_deadline_is_killed_and_is_an_error() {
        let step = Step::new("/bin/sleep", &["5"]);
        let started = Instant::now();
        let error = run_step(&step, Duration::from_millis(200)).expect_err("timed out");
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(
            started.elapsed() < Duration::from_secs(3),
            "it was not waited out"
        );
    }

    #[test]
    fn met_019_a_step_gets_only_path_and_its_own_variables() {
        let dir = tempfile::tempdir().expect("a folder");
        let out = dir.path().join("env");
        let mut step = Step::new("/bin/sh", &["-c"]);
        step.args.push(format!("env > {}", out.display()));
        step.env
            .push(("XDG_RUNTIME_DIR", "/run/user/1000".to_owned()));
        assert!(run_step(&step, STEP_DEADLINE).expect("ran"));
        let seen = std::fs::read_to_string(&out).expect("written");
        assert!(seen.contains("XDG_RUNTIME_DIR=/run/user/1000\n"), "{seen}");
        assert!(!seen.contains("HOME="), "{seen}");
    }

    #[test]
    fn met_019_a_failed_step_runs_again_after_each_pause_until_they_are_spent() {
        let dir = tempfile::tempdir().expect("a folder");
        let count = dir.path().join("count");
        let mut step = Step::new("/bin/sh", &["-c"]);
        step.args
            .push(format!("echo run >> {}; exit 5", count.display()));
        assert!(!run_step(&step, STEP_DEADLINE).expect("ran"));
        step.retries = &[SHORT; 2];
        assert!(!run_step(&step, STEP_DEADLINE).expect("ran"));
        let runs = std::fs::read_to_string(&count).expect("written");
        assert_eq!(
            runs.lines().count(),
            4,
            "once without retries, three times with two"
        );
    }

    #[test]
    fn met_019_a_bootstrap_that_answers_eio_twice_succeeds_at_the_third_try() {
        let dir = tempfile::tempdir().expect("a folder");
        let count = dir.path().join("count");
        let mut step = Step::new("/bin/sh", &["-c"]);
        step.args.push(format!(
            "echo run >> {0}; [ $(wc -l < {0}) -ge 3 ] || exit 5",
            count.display()
        ));
        step.retries = &[SHORT; 4];
        assert!(run_step(&step, STEP_DEADLINE).expect("ran"));
        let runs = std::fs::read_to_string(&count).expect("written");
        assert_eq!(
            runs.lines().count(),
            3,
            "no run after the one that succeeded"
        );
    }
}
