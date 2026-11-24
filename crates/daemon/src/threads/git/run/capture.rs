//! Bounded output capture for a Git process group.

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{FolderId, Git, GitError, MAX_OUTPUT, Run, said};

const MAX_TRACE: usize = 256 * 1024;

impl Git {
    /// Runs `git` and gives what it wrote to standard output.
    ///
    /// # Errors
    /// A [`GitError`] when `git` did not start, did not end in time, wrote too much, or ended
    /// with a code that is not 0.
    pub async fn output(&self, run: Run<'_>) -> Result<Vec<u8>, GitError> {
        let finished = self.finished(run, Capture::Diagnostic).await?;
        if finished.code == Some(0) {
            Ok(finished.out)
        } else {
            Err(finished.failure())
        }
    }

    /// Returns bounded Trace2 evidence with the ID of the child that was started.
    /// The evidence is for local validation and must stay out of logs and replies.
    pub(crate) async fn trace(&self, run: Run<'_>) -> Result<(u32, Vec<u8>), GitError> {
        let run = run.with_env("GIT_TRACE2_EVENT", "2");
        let finished = self.finished(run, Capture::Trace).await?;
        if finished.code != Some(0) {
            return Err(GitError::Failed {
                code: finished.code,
                said: "Git ended before the transport could be confirmed".to_owned(),
            });
        }
        finished.pid.zip(finished.err).ok_or(GitError::TooLarge)
    }

    /// As [`Git::output`], as text with the last line end removed.
    ///
    /// # Errors
    /// As [`Git::output`].
    pub async fn text(&self, run: Run<'_>) -> Result<String, GitError> {
        let out = self.output(run).await?;
        Ok(String::from_utf8_lossy(&out).trim_end().to_owned())
    }

    /// Runs `git` for a question whose answer is its exit code: 0 is yes, 1 is no.
    ///
    /// # Errors
    /// A [`GitError`] for each other code and each failure of the run.
    pub async fn yes(&self, run: Run<'_>) -> Result<bool, GitError> {
        let finished = self.finished(run, Capture::Diagnostic).await?;
        match finished.code {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(finished.failure()),
        }
    }

    async fn finished(&self, run: Run<'_>, stderr: Capture) -> Result<Finished, GitError> {
        // Directly before the run: the work folder is still the one that the runner made, not
        // a folder or a link that an agent put in its place (P12).
        if let Some((folder, made)) = &self.folder
            && FolderId::of(folder) != Some(*made)
        {
            return Err(GitError::Replaced);
        }
        let mut child = self.command(&run).spawn().map_err(GitError::Start)?;
        let group = child.id();
        let input = child.stdin.take().zip(run.input);
        let (stdout, diagnostic) = (child.stdout.take(), child.stderr.take());
        let collect = async {
            let write = async {
                if let Some((mut pipe, bytes)) = input {
                    // A git that exits before consuming stdin is reported by its status.
                    let _ = pipe.write_all(&bytes).await;
                }
            };
            let ((), out) = tokio::join!(write, Capture::Diagnostic.read(stdout));
            let status = child.wait().await.map_err(GitError::Start)?;
            Ok((status.code(), out?))
        };
        let work = async { tokio::try_join!(collect, stderr.read(diagnostic)) };
        let done = tokio::time::timeout(run.time, work).await;
        let whole = matches!(&done, Ok(Ok(((_, Some(_)), _))));
        if !whole {
            // A hook or a helper that `git` started ends with it, not only `git` itself.
            end_group(group);
        }
        let ((code, out), err) = done.map_err(|_| GitError::TimedOut(run.time.as_secs()))??;
        let out = out.ok_or(GitError::TooLarge)?;
        Ok(Finished {
            pid: group,
            code,
            out,
            err,
        })
    }
}

struct Finished {
    pid: Option<u32>,
    code: Option<i32>,
    out: Vec<u8>,
    err: Option<Vec<u8>>,
}

impl Finished {
    fn failure(&self) -> GitError {
        let said = String::from_utf8_lossy(self.err.as_deref().unwrap_or_default());
        GitError::Failed {
            code: self.code,
            said: said::without_credentials(&said),
        }
    }
}

/// Ends each process of the group of a run that did not end well by itself.
#[cfg(unix)]
fn end_group(group: Option<u32>) {
    let pid = group
        .and_then(|pid| i32::try_from(pid).ok())
        .and_then(rustix::process::Pid::from_raw);
    if let Some(pid) = pid {
        // A group that ended already has nothing to end.
        let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
    }
}

/// On Windows `kill_on_drop` ends `git`; a process group is a thing of Unix.
#[cfg(not(unix))]
const fn end_group(_group: Option<u32>) {}

#[derive(Clone, Copy)]
enum Capture {
    Diagnostic,
    Trace,
}

impl Capture {
    async fn read(
        self,
        pipe: Option<impl tokio::io::AsyncRead + Unpin>,
    ) -> Result<Option<Vec<u8>>, GitError> {
        let mut kept = Vec::new();
        let Some(pipe) = pipe else {
            return Ok(Some(kept));
        };
        let most = match self {
            Self::Diagnostic => MAX_OUTPUT,
            Self::Trace => MAX_TRACE,
        };
        let limit = u64::try_from(most).unwrap_or(u64::MAX);
        let mut pipe = pipe.take(limit.saturating_add(1));
        let read = pipe.read_to_end(&mut kept).await;
        if matches!(self, Self::Trace) && (read.is_err() || kept.len() > most) {
            return Err(GitError::TooLarge);
        }
        Ok((kept.len() <= most).then_some(kept))
    }
}
