//! One member of the group as a child process with a sealed environment and its own log file.

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::ExitStatus;
#[cfg(unix)]
use std::process::Stdio;
use std::time::Duration;

#[cfg(unix)]
use nix::sys::signal::{Signal, kill};
#[cfg(unix)]
use nix::unistd::Pid;
use tokio::io::{AsyncBufReadExt as _, AsyncRead, BufReader};
#[cfg(unix)]
use tokio::net::unix::pipe;
#[cfg(unix)]
use tokio::process::Command;
use tokio::sync::{mpsc, watch};

use crate::log_file::LogFile;

#[cfg(all(test, unix))]
mod tests;
#[cfg(windows)]
mod windows;

const LINE_LIMIT: usize = 16 * 1024;

const LAST_LINES: Duration = Duration::from_millis(500);

/// What to run; the child gets `env` and nothing else of this program's environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    pub name: &'static str,
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub env: Vec<(&'static str, OsString)>,
}

#[derive(Debug, thiserror::Error)]
pub enum ChildError {
    #[error("{name} cannot be started ({kind})")]
    Start {
        name: &'static str,
        kind: io::ErrorKind,
    },
    #[error("{name} ended ({status})")]
    Ended {
        name: &'static str,
        status: ExitStatus,
    },
    #[error("{name} was not ready in {} s", limit.as_secs())]
    NotReady { name: &'static str, limit: Duration },
    #[error("a signal cannot be sent to {name}")]
    Signal { name: &'static str },
}

impl ChildError {
    /// The exit code of a member that ended by itself.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        match self {
            Self::Ended { status, .. } => status.code(),
            _ => None,
        }
    }
}

/// The request to stop the group: SIGTERM or SIGINT in the program, a handle in a test.
#[derive(Debug, Clone)]
pub struct Stop(watch::Receiver<bool>);

impl Stop {
    #[must_use]
    pub fn new() -> (watch::Sender<bool>, Self) {
        let (ask, asked) = watch::channel(false);
        (ask, Self(asked))
    }

    /// A request that SIGTERM or SIGINT makes. A signal that comes before anybody waits is kept.
    ///
    /// # Errors
    /// When the signals cannot be watched.
    #[cfg(unix)]
    pub fn on_signals() -> io::Result<Self> {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = signal(SignalKind::terminate())?;
        let mut interrupt = signal(SignalKind::interrupt())?;
        let (ask, stop) = Self::new();
        tokio::spawn(async move {
            tokio::select! {
                _ = terminate.recv() => {}
                _ = interrupt.recv() => {}
            }
            ask.send_replace(true);
            // Kept: a closed channel would read as "never asked" to a late clone.
            ask.closed().await;
        });
        Ok(stop)
    }

    #[must_use]
    pub fn is_asked(&self) -> bool {
        *self.0.borrow()
    }

    /// Returns when the stop is asked for. Never returns when the asking side is gone.
    pub async fn asked(&mut self) {
        if self.0.wait_for(|asked| *asked).await.is_err() {
            std::future::pending::<()>().await;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stopped {
    /// It ended by itself, before or after SIGTERM.
    Ended(ExitStatus),
    Killed,
}

#[derive(Debug)]
pub struct Child {
    name: &'static str,
    #[cfg(unix)]
    process: Pid,
    #[cfg(windows)]
    input: Option<tokio::process::ChildStdin>,
    #[cfg(windows)]
    job: Option<anima_windows::job::Job>,
    exit: watch::Receiver<Option<ExitStatus>>,
    lines: Option<mpsc::UnboundedReceiver<String>>,
}

impl Child {
    /// Starts `spec`; `ended` gets the member's name when it ends.
    ///
    /// The member stays in this program's process group, so launchd or systemd kills it with this
    /// program; Ctrl-C in a terminal then reaches every member at once.
    ///
    /// # Errors
    /// [`ChildError::Start`] when the program cannot be run.
    #[cfg(unix)]
    pub fn start(
        spec: &Spec,
        log: LogFile,
        ended: mpsc::UnboundedSender<&'static str>,
    ) -> Result<Self, ChildError> {
        let name = spec.name;
        let failed = |error: io::Error| ChildError::Start {
            name,
            kind: error.kind(),
        };
        // One pipe for both outputs keeps the order, so a notice on stderr is read before a later `ready`.
        let (output, input) = std::io::pipe().map_err(failed)?;
        let mut child = Command::new(&spec.program)
            .args(&spec.args)
            .env_clear()
            .envs(spec.env.iter().map(|(key, value)| (key, value)))
            .current_dir("/")
            .stdin(Stdio::null())
            .stdout(input.try_clone().map_err(failed)?)
            .stderr(input)
            .kill_on_drop(true)
            .spawn()
            .map_err(failed)?;
        let process = child
            .id()
            .and_then(|id| i32::try_from(id).ok())
            .map(Pid::from_raw)
            .ok_or_else(|| failed(io::ErrorKind::Other.into()))?;
        let output = pipe::Receiver::from_owned_fd(output.into()).map_err(failed)?;
        let (line, lines) = mpsc::unbounded_channel();
        tokio::spawn(copy_lines(output, log, line));
        let (report, exit) = watch::channel(None);
        tokio::spawn(async move {
            // An error here means the child cannot be waited for; the group treats that as ended.
            let status = child.wait().await.unwrap_or_default();
            report.send_replace(Some(status));
            let _ = ended.send(name);
        });
        Ok(Self {
            name,
            process,
            exit,
            lines: Some(lines),
        })
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    #[must_use]
    pub fn ended(&self) -> Option<ExitStatus> {
        *self.exit.borrow()
    }

    /// Reads the member's output lines until `pick` takes one.
    ///
    /// # Errors
    /// [`ChildError::Ended`] when the member ends first, [`ChildError::NotReady`] after `limit`.
    pub async fn line_where<T>(
        &mut self,
        limit: Duration,
        mut pick: impl FnMut(&str) -> Option<T>,
    ) -> Result<T, ChildError> {
        let name = self.name;
        let mut exit = self.exit.clone();
        let read = async {
            while let Some(lines) = self.lines.as_mut() {
                // A process that the member started can keep the output open after the member ended.
                let line = tokio::select! {
                    biased;
                    line = lines.recv() => line,
                    _ = wait(&mut exit) => break,
                };
                match line.as_deref().map(&mut pick) {
                    Some(Some(picked)) => return Ok(picked),
                    Some(None) => {}
                    None => self.lines = None,
                }
            }
            let status = wait(&mut exit).await;
            // What the member wrote last can still be on its way.
            while let Some(lines) = self.lines.as_mut()
                && let Ok(Some(line)) = tokio::time::timeout(LAST_LINES, lines.recv()).await
            {
                if let Some(picked) = pick(&line) {
                    return Ok(picked);
                }
            }
            Err(ChildError::Ended { name, status })
        };
        tokio::time::timeout(limit, read)
            .await
            .unwrap_or(Err(ChildError::NotReady { name, limit }))
    }

    /// Stops reading the output for a line; it still goes to the log.
    pub fn stop_reading(&mut self) {
        self.lines = None;
    }

    /// Waits for the member to end by itself.
    ///
    /// # Errors
    /// [`ChildError::NotReady`] after `limit`.
    pub async fn end_within(&mut self, limit: Duration) -> Result<ExitStatus, ChildError> {
        tokio::time::timeout(limit, wait(&mut self.exit))
            .await
            .map_err(|_| ChildError::NotReady {
                name: self.name,
                limit,
            })
    }

    /// SIGTERM, then `limit` to end, then SIGKILL. A member's own children are its to stop; the
    /// database program stops a PostgreSQL server that a killed copy left at its next start.
    ///
    /// # Errors
    /// [`ChildError::Signal`] when a live member cannot be signalled.
    #[cfg(unix)]
    pub async fn stop(mut self, limit: Duration) -> Result<Stopped, ChildError> {
        if let Some(status) = self.ended() {
            return Ok(Stopped::Ended(status));
        }
        self.signal(Signal::SIGTERM)?;
        if let Ok(status) = self.end_within(limit).await {
            return Ok(Stopped::Ended(status));
        }
        self.signal(Signal::SIGKILL)?;
        wait(&mut self.exit).await;
        Ok(Stopped::Killed)
    }

    /// A process that ended between the look and the signal is not an error.
    #[cfg(unix)]
    fn signal(&self, signal: Signal) -> Result<(), ChildError> {
        if self.ended().is_some() {
            return Ok(());
        }
        match kill(self.process, signal) {
            Ok(()) | Err(nix::errno::Errno::ESRCH) => Ok(()),
            Err(_) => Err(ChildError::Signal { name: self.name }),
        }
    }
}

async fn wait(exit: &mut watch::Receiver<Option<ExitStatus>>) -> ExitStatus {
    match exit.wait_for(Option::is_some).await {
        Ok(status) => status.unwrap_or_default(),
        // The task that waits for the child is gone, so the child is.
        Err(_) => ExitStatus::default(),
    }
}

/// Reads to the end also when nobody takes the lines: a full pipe would stop the member.
async fn copy_lines(
    output: impl AsyncRead + Unpin,
    mut log: LogFile,
    line: mpsc::UnboundedSender<String>,
) {
    let mut output = BufReader::new(output);
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        match output.read_until(b'\n', &mut bytes).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {}
        }
        let text = String::from_utf8_lossy(&bytes[..bytes.len().min(LINE_LIMIT)]);
        let text = text.trim_end_matches(['\n', '\r']);
        // A log that cannot be written must not stop the member.
        let _ = log.line(text);
        let _ = line.send(text.to_owned());
    }
}
