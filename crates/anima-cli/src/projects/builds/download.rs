//! Keep the destination private and publish it only after a verified transfer.
use std::{
    io,
    path::{Path, PathBuf},
};

use uuid::Uuid;

use crate::{failure::Failure, invocation::Invocation};

#[derive(Debug, clap::Args)]
pub struct DownloadArgs {
    /// The source build ID.
    job: Uuid,
    /// The successful attempt number returned by build status.
    #[arg(value_parser = clap::value_parser!(u32).range(1..))]
    attempt: u32,
    /// A new local file to receive the archive.
    #[arg(long)]
    output: PathBuf,
}

impl DownloadArgs {
    pub(super) async fn run(self, invocation: &Invocation) -> Result<(), Failure> {
        let hub = invocation.hub()?;
        let path = self.output;
        let folder = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        if path.symlink_metadata().is_ok() {
            return Err(exists(&path));
        }
        let temp = tempfile::Builder::new()
            .prefix(".anima-source-")
            .tempfile_in(folder)
            .map_err(|error| written(&path, &error))?;
        #[cfg(windows)]
        anima_windows::files::protect(temp.path()).map_err(|error| written(&path, &error))?;
        let mut stream =
            tokio::fs::File::from_std(temp.reopen().map_err(|error| written(&path, &error))?);
        let receipt = hub
            .download_source_artifact(self.job, self.attempt, &mut stream)
            .await?;
        stream
            .sync_all()
            .await
            .map_err(|error| written(&path, &error))?;
        drop(stream);
        temp.persist_noclobber(&path).map_err(|error| {
            if error.error.kind() == io::ErrorKind::AlreadyExists {
                exists(&path)
            } else {
                written(&path, &error.error)
            }
        })?;
        #[cfg(unix)]
        tokio::fs::File::open(folder)
            .await
            .map_err(|error| written(&path, &error))?
            .sync_all()
            .await
            .map_err(|error| written(&path, &error))?;
        if invocation.output.json() {
            invocation
                .output
                .raw(&serde_json::json!({ "path": path, "artifact": receipt }))?;
        } else {
            invocation.output.message(&format!(
                "Saved {} ({} bytes).",
                path.display(),
                receipt.bytes
            ));
        }
        Ok(())
    }
}

fn exists(path: &Path) -> Failure {
    Failure::Conflict {
        message: format!("{} already exists.", path.display()),
    }
}

fn written(path: &Path, error: &io::Error) -> Failure {
    Failure::Write {
        reason: format!("{}: {error}", path.display()),
    }
}
