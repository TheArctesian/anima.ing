//! Streams an exact, sealed artifact to an already-authorized local owner.
use super::{
    super::{Error, Result, command},
    artifact, store,
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{Semaphore, SemaphorePermit};
use uuid::Uuid;

static EXPORTS: Semaphore = Semaphore::const_new(1);

pub struct Prepared {
    _attempt: std::fs::File,
    _permit: SemaphorePermit<'static>,
    file: std::fs::File,
    pub bytes: u64,
}

pub async fn stream(id: Uuid, archive_digest: &str, manifest_digest: &str) -> Result<()> {
    open(id, archive_digest, manifest_digest)
        .await?
        .write_to(&mut tokio::io::stdout())
        .await
}

pub async fn open(id: Uuid, archive_digest: &str, manifest_digest: &str) -> Result<Prepared> {
    let permit = EXPORTS
        .try_acquire()
        .map_err(|_| Error::Refused("Another artifact export is active."))?;
    let attempt_lock = store::lock(id)?;
    let attempt = store::required(id)?;
    let result = attempt
        .result
        .filter(|result| {
            attempt.phase == "verified"
                && result["integrityVerified"] == true
                && result["archiveDigest"] == archive_digest
                && result["manifestDigest"] == manifest_digest
        })
        .ok_or(Error::Refused(
            "The requested artifact is not the verified build result.",
        ))?;
    let work = store::base(id).join("work");
    let sealed = command::run(
        "btrfs",
        &[
            "property",
            "get",
            "-ts",
            work.to_str()
                .ok_or(Error::Refused("Invalid artifact path."))?,
            "ro",
        ],
    )
    .await?;
    if sealed.trim() != "ro=true" {
        return Err(Error::Refused("The build artifact is not sealed."));
    }
    tokio::task::spawn_blocking(move || {
        let (verified, file) = artifact::open_verified(&work.join("image.oci"), &result)?;
        Ok(Prepared {
            _attempt: attempt_lock,
            _permit: permit,
            file,
            bytes: verified.bytes,
        })
    })
    .await
    .map_err(|_| Error::Refused("Artifact verification did not complete."))?
}

impl Prepared {
    pub async fn write_to(self, output: &mut (impl AsyncWrite + Unpin)) -> Result<()> {
        let mut file = tokio::fs::File::from_std(self.file).take(self.bytes + 1);
        let transferred = tokio::time::timeout(Duration::from_mins(30), async {
            let bytes = tokio::io::copy(&mut file, output).await?;
            output.flush().await?;
            Ok::<u64, std::io::Error>(bytes)
        })
        .await
        .map_err(|_| Error::Refused("The artifact export exceeded its deadline."))??;
        if transferred != self.bytes {
            return Err(Error::Refused("The artifact export was incomplete."));
        }
        Ok(())
    }
}
