//! Bounded output capture for a Git process group.

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{FolderId, Git, GitError, Run, output, said};

const MAX_TRACE: usize = 256 * 1024;

impl Git {
    /// Runs `git` and gives what it wrote to standard output.
    ///
    /// # Errors
    /// A [`GitError`] when `git` did not start, did not end in time, wrote too much, or ended
    /// with a code that is not 0.
    pub async fn output(&self, run: Run<'_>) -> Result<Vec<u8>, GitError> {
        let (code, out, said) = self.finished(run).await?;
        if code == Some(0) {
            Ok(out)
        } else {
            Err(GitError::Failed { code, said })
        }
    }

    /// Returns bounded Trace2 evidence with the ID of the child that was started.
    /// The evidence is for local validation and must stay out of logs and replies.
    pub(crate) async fn trace(&self, run: Run<'_>) -> Result<(u32, Vec<u8>), GitError> {
        let run = run.with_env("GIT_TRACE2_EVENT", "2");
        let finished = self.capture(run, Capture::Trace).await?;
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
        match self.finished(run).await? {
            (Some(0), ..) => Ok(true),
            (Some(1), ..) => Ok(false),
            (code, _, said) => Err(GitError::Failed { code, said }),
        }
    }

    /// Return the exit status, output and redacted diagnostics for the LFS storage check.
    pub(in crate::threads::git) async fn finished(
        &self,
        run: Run<'_>,
    ) -> Result<(Option<i32>, Vec<u8>, String), GitError> {
        let captured = self.capture(run, Capture::Diagnostic).await?;
        let stderr = String::from_utf8_lossy(captured.err.as_deref().unwrap_or_default());
        Ok((
            captured.code,
            captured.out,
            said::without_credentials(&stderr),
        ))
    }

    async fn capture(&self, run: Run<'_>, stderr: Capture) -> Result<Finished, GitError> {
        // Directly before the run: the work folder is still the one that the runner made, not
        // a folder or a link that an agent put in its place (P12).
        if let Some((folder, made)) = &self.folder
            && FolderId::of(folder) != Some(*made)
        {
            return Err(GitError::Replaced);
        }
        self.check_records(&run)?;
        let mut group = ProcessGroup(self.command(&run).spawn().map_err(GitError::Start)?);
        let pid = group.0.id();
        let input = group.0.stdin.take().zip(run.input);
        let (stdout, diagnostic) = (group.0.stdout.take(), group.0.stderr.take());
        let prefix = run.output_prefix;
        let collect = async {
            let write = async {
                if let Some((mut pipe, bytes)) = input {
                    // A git that exits before consuming stdin is reported by its status.
                    let _ = pipe.write_all(&bytes).await;
                }
            };
            let ((), out) = tokio::join!(write, output::held(stdout, prefix));
            out.ok_or(GitError::TooLarge)
        };
        let work = async {
            let (out, err) = tokio::try_join!(collect, stderr.read(diagnostic))?;
            // Keep the leader unreaped while a descendant can still hold the pipes.
            let status = group.0.wait().await.map_err(GitError::Start)?;
            Ok(Finished {
                pid,
                code: status.code(),
                out,
                err,
            })
        };
        tokio::time::timeout(run.time, work)
            .await
            .map_err(|_| GitError::TimedOut(run.time.as_secs()))?
    }
}

struct Finished {
    pid: Option<u32>,
    code: Option<i32>,
    out: Vec<u8>,
    err: Option<Vec<u8>>,
}

struct ProcessGroup(tokio::process::Child);

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        // Child::id returns None after wait reaps the leader. Otherwise the child still
        // owns the PID. Signal the group before dropping the child and handing it to Tokio.
        #[cfg(unix)]
        if let Some(pid) = self
            .0
            .id()
            .and_then(|pid| i32::try_from(pid).ok())
            .and_then(rustix::process::Pid::from_raw)
        {
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
        }
    }
}

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
        if matches!(self, Self::Diagnostic) {
            return Ok(output::held(pipe, None).await);
        }
        let mut kept = Vec::new();
        let Some(pipe) = pipe else {
            return Ok(Some(kept));
        };
        let limit = u64::try_from(MAX_TRACE).unwrap_or(u64::MAX);
        let mut pipe = pipe.take(limit.saturating_add(1));
        if pipe.read_to_end(&mut kept).await.is_err() || kept.len() > MAX_TRACE {
            return Err(GitError::TooLarge);
        }
        Ok(Some(kept))
    }
}
