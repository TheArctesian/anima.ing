//! Runs the processes of one build step (BLD-027, BLD-029, BLD-044); the lock is never held across an `await`.

use std::{
    ffi::OsString,
    path::Path,
    process::{ExitStatus, Stdio},
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use tokio::io::{AsyncRead, AsyncReadExt};

use super::{
    error::BuilderError,
    lines::Lines,
    plan::{Command, Kept},
    reaper,
};

pub type Sink = Arc<dyn Fn(String) + Send + Sync>;

const KEPT_LINES_MAX: usize = 2000;
const DRAIN: Duration = Duration::from_secs(5);
const CHUNK: usize = 8192;

#[derive(Debug, Default)]
struct Slot {
    busy: bool,
    group: Option<i32>,
    #[cfg(windows)]
    job: Option<anima_windows::job::Job>,
    cancelled: bool,
    marker: Option<String>,
}

fn next_marker() -> String {
    static COUNT: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}",
        std::process::id(),
        COUNT.fetch_add(1, Ordering::Relaxed)
    )
}

/// The one build step that may run at a time on this computer (BLD-029), and its process group.
#[derive(Debug, Default)]
pub struct Running {
    slot: Mutex<Slot>,
}

pub struct Claim<'a> {
    running: &'a Running,
    sink: Sink,
    marker: String,
}

fn kill_group(group: Option<i32>) {
    #[cfg(unix)]
    if let Some(group) = group {
        let _ = nix::sys::signal::killpg(
            nix::unistd::Pid::from_raw(group),
            nix::sys::signal::Signal::SIGKILL,
        );
    }
    #[cfg(not(unix))]
    let _ = group;
}

impl Running {
    fn with_slot<T>(&self, change: impl FnOnce(&mut Slot) -> T) -> T {
        change(&mut self.slot.lock().unwrap_or_else(PoisonError::into_inner))
    }

    pub fn claim(&self, sink: Sink) -> Result<Claim<'_>, BuilderError> {
        self.with_slot(|slot| {
            if slot.busy {
                return Err(BuilderError::Busy);
            }
            reaper::become_subreaper();
            let marker = next_marker();
            *slot = Slot {
                busy: true,
                marker: Some(marker.clone()),
                ..Slot::default()
            };
            Ok(Claim {
                running: self,
                sink,
                marker,
            })
        })
    }

    pub fn cancel(&self) {
        self.with_slot(|slot| {
            if slot.busy {
                slot.cancelled = true;
                kill_group(slot.group);
                #[cfg(windows)]
                drop(slot.job.take());
                if let Some(marker) = &slot.marker {
                    reaper::kill_marked(marker);
                }
            }
        });
    }
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        self.running.with_slot(|slot| *slot = Slot::default());
    }
}

async fn read_lines(stream: Option<impl AsyncRead + Unpin>, sink: Sink, keep: bool) -> Vec<String> {
    let mut kept = Vec::new();
    let Some(mut stream) = stream else {
        return kept;
    };
    let mut lines = Lines::default();
    let mut buffer = vec![0; CHUNK];
    let mut pass = |line: String| {
        if keep && kept.len() < KEPT_LINES_MAX {
            kept.push(line.clone());
        }
        sink(line);
    };
    while let Ok(read @ 1..) = stream.read(&mut buffer).await {
        lines.push(&buffer[..read]).into_iter().for_each(&mut pass);
    }
    lines.finish().into_iter().for_each(&mut pass);
    kept
}

impl Claim<'_> {
    /// A process that left the step and still holds its output (a compiler cache server) does not fail it.
    pub async fn run(
        &self,
        command: &Command,
        program: &Path,
        env: Vec<(OsString, OsString)>,
    ) -> Result<(), BuilderError> {
        self.finish(command, program, env).await.map(drop)
    }

    /// A kept stream that did not close fails the step: for `git status` or `codesign -d --entitlements`, empty is the passing answer.
    pub async fn read(
        &self,
        command: &Command,
        program: &Path,
        env: Vec<(OsString, OsString)>,
    ) -> Result<Vec<String>, BuilderError> {
        self.finish(command, program, env)
            .await?
            .ok_or(BuilderError::StepFailed)
    }

    async fn finish(
        &self,
        command: &Command,
        program: &Path,
        mut env: Vec<(OsString, OsString)>,
    ) -> Result<Option<Vec<String>>, BuilderError> {
        env.push((reaper::MARKER.into(), self.marker.clone().into()));
        let mut process = tokio::process::Command::new(program);
        process
            .args(&command.args)
            .current_dir(&command.cwd)
            .env_clear()
            .envs(env)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        process.process_group(0);
        #[cfg(windows)]
        process.creation_flags(anima_windows::job::SUSPENDED);
        let group = self.running.with_slot(|slot| {
            if slot.cancelled {
                return Err(BuilderError::Cancelled);
            }
            let mut child = process.spawn().map_err(|_| BuilderError::Failed)?;
            #[cfg(unix)]
            {
                slot.group = child.id().and_then(|id| i32::try_from(id).ok());
            }
            #[cfg(windows)]
            {
                let pid = child.id().ok_or(BuilderError::Failed)?;
                slot.job =
                    Some(anima_windows::job::Job::start(pid).map_err(|_| BuilderError::Failed)?);
            }
            Ok((child.stdout.take(), child.stderr.take(), child))
        });
        let (stdout, stderr, mut child) = group?;
        let keep_errors = command.kept == Kept::Errors;
        let out = tokio::spawn(read_lines(stdout, Arc::clone(&self.sink), !keep_errors));
        let err = tokio::spawn(read_lines(stderr, Arc::clone(&self.sink), keep_errors));
        let status = child.wait().await;
        // Anything the step left, in its group or out of it, would hold the output open.
        let group = self.running.with_slot(|slot| {
            #[cfg(windows)]
            drop(slot.job.take());
            let group = slot.group.take();
            kill_group(group);
            group
        });
        let marker = self.marker.clone();
        let _ = tokio::task::spawn_blocking(move || reaper::sweep(&marker, group)).await;
        let cancelled = self.running.with_slot(|slot| slot.cancelled);
        // Both streams drain at once, so a stray holding both costs one `DRAIN`, not two.
        let (out, err) = tokio::join!(
            tokio::time::timeout(DRAIN, out),
            tokio::time::timeout(DRAIN, err)
        );
        let out = out.ok().and_then(Result::ok);
        let err = err.ok().and_then(Result::ok);
        outcome(cancelled, status.ok(), if keep_errors { err } else { out })
    }
}

fn outcome(
    cancelled: bool,
    status: Option<ExitStatus>,
    kept: Option<Vec<String>>,
) -> Result<Option<Vec<String>>, BuilderError> {
    if cancelled {
        return Err(BuilderError::Cancelled);
    }
    match status {
        Some(status) if status.success() => Ok(kept),
        _ => Err(BuilderError::StepFailed),
    }
}
