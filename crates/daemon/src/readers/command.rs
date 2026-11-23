//! Runs one fixed read-only command: no shell, a time limit, an output cap, and its process group dies with it (MET-002, CAP-022).

use std::io;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

#[cfg(unix)]
use rustix::process::{Pid, Signal, kill_process_group};
use tokio::io::{AsyncRead, AsyncReadExt as _};
use tokio::process::{Child, Command};
use tokio::time::timeout;

const OUTPUT_LIMIT: u64 = 1024 * 1024;

/// The only search path a command gets: the daemon's own environment is not passed on.
#[cfg(unix)]
const PATH: &str = "/run/current-system/sw/bin:/usr/sbin:/usr/bin:/sbin:/bin";

const EXIT_GRACE: Duration = Duration::from_secs(1);

/// A program started in the background may hold the output open; it is not waited for.
const DRAIN: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, Debug)]
pub struct FixedCommand {
    pub program: &'static str,
    pub args: &'static [&'static str],
    pub limit: Duration,
    /// `intel_gpu_top -J` never ends by itself: its output up to the limit is the answer.
    pub keep_cut_output: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("{program} could not be started: {source}")]
    Start {
        program: &'static str,
        source: io::Error,
    },
    #[error("{program} did not finish in time")]
    Timeout { program: &'static str },
    #[error("{program} failed")]
    Failed { program: &'static str },
}

impl FixedCommand {
    /// The command gets only a fixed `PATH` and `LC_ALL=C`, the number format its parser expects.
    ///
    /// # Errors
    /// [`CommandError`] when it cannot start, runs past its limit (unless `keep_cut_output`) or fails.
    pub async fn run(&self) -> Result<Vec<u8>, CommandError> {
        let mut command = Command::new(self.program);
        command.args(self.args).env_clear().env("LC_ALL", "C");
        #[cfg(unix)]
        command.env("PATH", PATH);
        #[cfg(windows)]
        windows_environment(&mut command);
        let bounds = Bounds {
            limit: self.limit,
            keep_cut_output: self.keep_cut_output,
            output_limit: OUTPUT_LIMIT,
        };
        capture(&mut command, self.program, bounds).await
    }
}

#[cfg(windows)]
fn windows_environment(command: &mut Command) {
    use std::{env, path::PathBuf};
    let system = env::var_os("SystemRoot")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    let mut folders = Vec::new();
    if let Some(system) = system {
        folders.push(system.join("System32"));
    }
    if let Some(programs) = env::var_os("ProgramFiles")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        folders.push(programs.join("NVIDIA Corporation/NVSMI"));
    }
    command.env("PATH", env::join_paths(folders).unwrap_or_default());
    for name in ["SystemRoot", "WINDIR", "TEMP", "TMP"] {
        if let Some(value) = env::var_os(name) {
            command.env(name, value);
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub limit: Duration,
    pub keep_cut_output: bool,
    pub output_limit: u64,
}

/// Runs `command` within `bounds`; it and every process of its group die when it ends, times out or is dropped.
///
/// # Errors
/// [`CommandError`] when it cannot start, runs past its limit (unless `keep_cut_output`) or fails.
pub async fn capture(
    command: &mut Command,
    program: &'static str,
    bounds: Bounds,
) -> Result<Vec<u8>, CommandError> {
    #[cfg(unix)]
    command.process_group(0);
    #[cfg(windows)]
    command.creation_flags(anima_windows::job::SUSPENDED);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|source| CommandError::Start { program, source })?;
    // Declared after `child`, so it is dropped first: the group dies before the child is dropped.
    #[cfg(unix)]
    let mut group = Group::of(&child);
    #[cfg(windows)]
    let mut group = Group::of(&child).map_err(|source| CommandError::Start { program, source })?;
    let Some(stdout) = child.stdout.take() else {
        return Err(CommandError::Failed { program });
    };
    let mut output = Vec::new();
    let mut reader = stdout.take(bounds.output_limit);
    // A read appends as it goes, so a cut leaves the output so far in `output`.
    let ended = timeout(
        bounds.limit,
        until_exit(&mut child, &mut reader, &mut output),
    )
    .await;
    let Ok(exited) = ended else {
        group.kill();
        // Reaped here; if that fails, `kill_on_drop` ends it with `child`.
        child.kill().await.ok();
        return if bounds.keep_cut_output {
            Ok(output)
        } else {
            Err(CommandError::Timeout { program })
        };
    };
    match exited {
        Ok(status) if status.success() => Ok(output),
        _ => Err(CommandError::Failed { program }),
    }
}

async fn until_exit(
    child: &mut Child,
    reader: &mut (impl AsyncRead + Unpin),
    output: &mut Vec<u8>,
) -> io::Result<ExitStatus> {
    tokio::select! {
        read = reader.read_to_end(output) => {
            read?;
            timeout(EXIT_GRACE, child.wait()).await?
        }
        status = child.wait() => {
            let status = status?;
            if let Ok(read) = timeout(DRAIN, reader.read_to_end(output)).await {
                read?;
            }
            Ok(status)
        }
    }
}

#[cfg(unix)]
struct Group(Option<Pid>);

#[cfg(unix)]
impl Group {
    fn of(child: &Child) -> Self {
        let leader = child
            .id()
            .and_then(|id| i32::try_from(id).ok())
            .and_then(Pid::from_raw);
        Self(leader)
    }

    fn kill(&mut self) {
        if let Some(group) = self.0.take() {
            // The group is often empty by now, so "no such process" is the wanted outcome.
            kill_process_group(group, Signal::KILL).ok();
        }
    }
}

#[cfg(windows)]
struct Group(Option<anima_windows::job::Job>);

#[cfg(windows)]
impl Group {
    fn of(child: &Child) -> io::Result<Self> {
        let pid = child.id().ok_or(io::ErrorKind::NotFound)?;
        Ok(Self(Some(anima_windows::job::Job::start(pid)?)))
    }
    fn kill(&mut self) {
        if let Some(job) = self.0.take() {
            job.kill();
        }
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        self.kill();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    const fn command(program: &'static str, args: &'static [&'static str]) -> FixedCommand {
        FixedCommand {
            program,
            args,
            limit: Duration::from_millis(300),
            keep_cut_output: false,
        }
    }

    #[tokio::test]
    async fn met_002_a_command_gives_its_output_with_no_shell() {
        let out = command("echo", &["$HOME"]).run().await.expect("runs");
        assert_eq!(out, b"$HOME\n");
    }

    #[tokio::test]
    async fn met_002_a_missing_program_a_failure_and_a_timeout_are_errors_not_crashes() {
        assert!(matches!(
            command("anima-no-such-program", &[]).run().await,
            Err(CommandError::Start { .. })
        ));
        assert!(matches!(
            command("false", &[]).run().await,
            Err(CommandError::Failed { .. })
        ));
        assert!(matches!(
            command("sleep", &["5"]).run().await,
            Err(CommandError::Timeout { .. })
        ));
    }

    #[tokio::test]
    async fn met_016_a_command_that_never_ends_can_give_its_output_so_far() {
        let streaming = FixedCommand {
            keep_cut_output: true,
            ..command("tail", &["-f", "/etc/passwd"])
        };
        let out = streaming.run().await.expect("the cut output is the answer");
        assert!(out.len() > 1);
    }
}
