use super::{Error, Receipt, Store, disk};
use crate::clock::Clock;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};
use tempfile::TempDir;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    sync::OwnedSemaphorePermit,
};

// Drop order keeps the store lock and reservation until staging cleanup has finished.
struct Staged {
    directory: TempDir,
    _permit: OwnedSemaphorePermit,
    store: Store,
    receipt: Receipt,
}

pub struct Upload(Option<Staged>);

impl Drop for Upload {
    fn drop(&mut self) {
        if let Some(staged) = self.0.take() {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn_blocking(move || drop(staged));
            } else {
                drop(staged);
            }
        }
    }
}

/// Exact length, EOF and archive digest verified, but not yet retained or acknowledged.
pub struct Verified(Upload);

impl Upload {
    pub(super) fn new(
        store: Store,
        permit: OwnedSemaphorePermit,
        directory: TempDir,
        receipt: Receipt,
    ) -> Self {
        Self(Some(Staged {
            directory,
            _permit: permit,
            store,
            receipt,
        }))
    }

    /// Streams with bounded memory and a thirty-minute deadline; no receipt on partial input.
    ///
    /// # Errors
    /// Truncated, trailing, corrupt or stalled input; storage or synchronization failure.
    pub async fn receive(self, input: impl AsyncRead + Unpin) -> Result<Verified, Error> {
        tokio::time::timeout(Duration::from_mins(30), self.receive_inner(input))
            .await
            .map_err(|_| Error::Invalid)?
    }

    async fn receive_inner(self, mut input: impl AsyncRead + Unpin) -> Result<Verified, Error> {
        let staged = self.0.as_ref().ok_or(Error::Invalid)?;
        let offer = &staged.receipt.artifact;
        receive_exact(
            &staged.directory.path().join("archive.oci"),
            (offer.bytes, &offer.archive_digest),
            &mut input,
        )
        .await?;
        tokio::task::spawn_blocking(move || {
            let staged = self.0.as_ref().ok_or(Error::Invalid)?;
            disk::write_receipt(staged.directory.path(), &staged.receipt)?;
            Ok(Verified(self))
        })
        .await
        .map_err(|_| Error::Invalid)?
    }
}

pub(super) async fn receive_exact(
    path: &std::path::Path,
    (bytes, digest): (u64, &str),
    mut input: impl AsyncRead + Unpin,
) -> Result<(), Error> {
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .await?;
    let mut hasher = Sha256::new();
    let mut remaining = bytes;
    let mut buffer = vec![0_u8; 65536];
    while remaining != 0 {
        let wanted =
            usize::try_from(remaining.min(buffer.len() as u64)).map_err(|_| Error::Invalid)?;
        let count = input.read(&mut buffer[..wanted]).await?;
        if count == 0 {
            return Err(Error::Invalid);
        }
        hasher.update(&buffer[..count]);
        file.write_all(&buffer[..count]).await?;
        remaining -= count as u64;
    }
    if input.read(&mut buffer[..1]).await? != 0
        || super::model::encoded_digest(&hasher.finalize()) != digest
    {
        return Err(Error::Invalid);
    }
    file.flush().await?;
    file.sync_all().await?;
    Ok(())
}

impl Verified {
    /// The caller holds the job authorization through the rename and directory sync.
    pub(super) async fn retain_before(
        mut self,
        clock: Arc<dyn Clock>,
        deadline: DateTime<Utc>,
    ) -> Result<Option<Receipt>, Error> {
        let staged = self.0.0.take().ok_or(Error::Invalid)?;
        tokio::task::spawn_blocking(move || {
            let root = &staged.store.0.root;
            let destination = root.join(super::model::directory(&staged.receipt.artifact));
            if destination.try_exists()? {
                return Err(Error::Conflict);
            }
            if clock.now() >= deadline {
                return Ok(None);
            }
            std::fs::rename(staged.directory.path(), &destination)?;
            let sync = disk::sync(root);
            if clock.now() >= deadline {
                // Return this publication to the incoming area before releasing ownership.
                std::fs::rename(destination, staged.directory.path())?;
                disk::sync(root)?;
                return Ok(None);
            }
            sync?;
            Ok(Some(staged.receipt.clone()))
        })
        .await
        .map_err(|_| Error::Invalid)?
    }
}
