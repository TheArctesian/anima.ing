//! One command of the operating system's unit tool, run with a fixed environment and a limit.

use std::io;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// How long a command may take. A stop waits for the group to stop in order.
pub const STEP_LIMIT: Duration = Duration::from_secs(120);

const LAUNCHCTL: &str = "/bin/launchctl";
const SYSTEMCTL: &str = "/usr/bin/systemctl";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    #[cfg(windows)]
    pub(super) stop_folders: Option<crate::Folders>,
    pub program: &'static str,
    pub args: Vec<String>,
    /// A failure is not an error: the unit may not be loaded.
    pub may_fail: bool,
    /// While a run fails, wait the next of these pauses and run it again; empty: one run.
    /// `launchctl bootstrap` right after `bootout` answers an input/output error (5) for some
    /// seconds while launchd still takes the old service out.
    pub retries: &'static [Duration],
}

impl Step {
    fn new(program: &'static str, args: &[&str]) -> Self {
        Self {
            #[cfg(windows)]
            stop_folders: None,
            program,
            args: args.iter().map(|&arg| arg.to_owned()).collect(),
            may_fail: false,
            retries: &[],
        }
    }

    pub(super) fn launchctl(args: &[&str]) -> Self {
        Self::new(LAUNCHCTL, args)
    }

    pub(super) fn systemctl(args: &[&str]) -> Self {
        let mut all = vec!["--user"];
        all.extend_from_slice(args);
        Self::new(SYSTEMCTL, &all)
    }

    pub(super) const fn may_fail(mut self) -> Self {
        self.may_fail = true;
        self
    }

    pub(super) const fn retried_after(mut self, pauses: &'static [Duration]) -> Self {
        self.retries = pauses;
        self
    }
}

/// Runs `step` with only `env` and a fixed `PATH`; `false` when it exits with a failure.
///
/// # Errors
/// When the program cannot be run, or does not end in `limit` (it is killed).
pub fn run_step(step: &Step, env: &[(&'static str, String)], limit: Duration) -> io::Result<bool> {
    let mut ok = run_once(step, env, limit)?;
    for pause in step.retries {
        if ok {
            break;
        }
        thread::sleep(*pause);
        ok = run_once(step, env, limit)?;
    }
    Ok(ok)
}

fn run_once(step: &Step, env: &[(&'static str, String)], limit: Duration) -> io::Result<bool> {
    #[cfg(windows)]
    if let Some(folders) = &step.stop_folders {
        super::windows::stop(folders, limit)?;
    }
    let mut command = Command::new(step.program);
    #[cfg(windows)]
    if step.program == "schtasks.exe" {
        command = Command::new(super::windows::task_program()?);
    }
    command.env_clear();
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
        if let Some(root) = std::env::var_os("SystemRoot") {
            command.env("SystemRoot", root);
        }
    }
    let mut child = command
        .args(&step.args)
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .envs(env.iter().map(|(name, value)| (name, value)))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status.success());
        }
        if started.elapsed() > limit {
            child.kill()?;
            child.wait()?;
            return Err(io::ErrorKind::TimedOut.into());
        }
        thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::time::Duration;

    use super::{Step, run_step};

    const SOON: Duration = Duration::from_secs(20);

    #[test]
    fn a_command_tells_if_it_succeeded_and_sees_only_the_given_environment() {
        let folder = tempfile::tempdir().unwrap();
        let seen = folder.path().join("seen");
        let script = format!("/usr/bin/env > '{}'", seen.display());
        let step = Step::new("/bin/sh", &["-c", &script]);

        let done = run_step(&step, &[("ONLY", "this".into())], SOON);

        assert!(done.unwrap());
        let seen = std::fs::read_to_string(seen).unwrap();
        assert!(
            seen.contains("ONLY=this") && !seen.contains("HOME="),
            "{seen}"
        );
        assert!(!run_step(&Step::new("/bin/sh", &["-c", "exit 3"]), &[], SOON).unwrap());
    }

    #[test]
    fn a_step_with_retries_runs_again_until_it_succeeds_and_then_stops() {
        const SHORT: Duration = Duration::from_millis(10);
        let folder = tempfile::tempdir().unwrap();
        let count = folder.path().join("count");
        // Fails twice, as launchd did after a bootout on 2026-11-21, then succeeds.
        let script = format!(
            "echo run >> '{0}'; [ $(wc -l < '{0}') -ge 3 ] || exit 5",
            count.display()
        );
        let step = Step::new("/bin/sh", &["-c", &script]);

        assert!(!run_step(&step, &[], SOON).unwrap());
        std::fs::remove_file(&count).unwrap();
        assert!(run_step(&step.retried_after(&[SHORT; 4]), &[], SOON).unwrap());
        let runs = std::fs::read_to_string(&count).unwrap();
        assert_eq!(
            runs.lines().count(),
            3,
            "no run after the one that succeeded"
        );
    }

    #[test]
    fn a_step_that_always_fails_runs_once_more_per_pause() {
        const SHORT: Duration = Duration::from_millis(10);
        let folder = tempfile::tempdir().unwrap();
        let count = folder.path().join("count");
        let script = format!("echo run >> '{0}'; exit 5", count.display());
        let step = Step::new("/bin/sh", &["-c", &script]).retried_after(&[SHORT; 2]);

        assert!(!run_step(&step, &[], SOON).unwrap());
        let runs = std::fs::read_to_string(&count).unwrap();
        assert_eq!(runs.lines().count(), 3);
    }

    #[test]
    fn a_command_that_does_not_end_is_killed_at_the_limit() {
        let step = Step::new("/bin/sh", &["-c", "exec sleep 30"]);

        let error = run_step(&step, &[], Duration::from_millis(200)).unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    }
}

#[cfg(windows)]
impl Step {
    pub(super) fn task(args: &[&str]) -> Self {
        Self::new("schtasks.exe", args)
    }
    pub(super) fn stopping(mut self, folders: crate::Folders) -> Self {
        self.stop_folders = Some(folders);
        self
    }
}
