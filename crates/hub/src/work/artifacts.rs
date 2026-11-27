//! Private source-build retention, separate from public releases and extension attachments.
//! Open at startup; transfer I/O and directory syncing stay off the async executor.
pub(crate) mod checkpoints;
mod delivery;
mod disk;
mod download;
mod model;
mod read;
mod registry;
#[cfg(test)]
mod tests;
mod transfer;

use std::os::unix::fs::PermissionsExt;
use std::{
    fs::File,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use uuid::Uuid;

pub use daemon_core::artifact::{MAX_ARCHIVE_BYTES, Offer, Receipt};
pub use delivery::{Delivery, DeliveryError, Pending};
pub(crate) use download::router as download_router;
pub use transfer::{Upload, Verified};

const MAX_RETAINED: usize = 1024;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("private artifact storage is busy")]
    Busy,
    #[error("private artifact storage has insufficient capacity")]
    Capacity,
    #[error("artifact identity conflicts with retained output")]
    Conflict,
    #[error("the confirmed receipt or image digest does not match retained output")]
    Confirmation,
    #[error("artifact input or retained output is invalid")]
    Invalid,
    #[error("retained output does not identify a runnable OCI image")]
    Image,
    #[error("private artifact registry configuration is invalid")]
    RegistryConfig,
    #[error("private artifact registry did not confirm publication")]
    Registry,
    #[error("artifact storage permissions or layout are invalid")]
    Private,
    #[error("artifact storage I/O failed ({0})")]
    Io(io::ErrorKind),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error.kind())
    }
}

struct Folder {
    root: PathBuf,
    lock: File,
    quota: u64,
    reserve: u64,
    turn: Arc<Semaphore>,
    readers: Arc<Semaphore>,
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = self.lock.unlock();
    }
}

#[derive(Clone)]
pub struct Store(Arc<Folder>, Option<Arc<registry::Publisher>>);

/// A replay is reverified before it returns the original receipt.
pub enum Intake {
    Retained(Receipt),
    Upload(Upload),
}

impl Store {
    /// Opens one exclusively owned, private directory; recovers incomplete uploads.
    /// This blocking startup operation must not run on an async executor thread.
    ///
    /// # Errors
    /// Unsafe permissions/layout, another owner, invalid quota or filesystem failure.
    pub fn open(root: &Path, quota: u64, reserve: u64) -> Result<Self, Error> {
        if quota == 0 || reserve == 0 || !root.is_absolute() || root.parent().is_none() {
            return Err(Error::Invalid);
        }
        let lock = disk::open(root)?;
        disk::recover(root)?;
        disk::inventory(root)?;
        let folder = Arc::new(Folder {
            root: root.to_owned(),
            lock,
            quota,
            reserve,
            turn: Arc::new(Semaphore::new(1)),
            readers: Arc::new(Semaphore::new(2)),
        });
        Ok(Self(folder, None))
    }

    /// Configure a single administrator-owned private image destination.
    ///
    /// # Errors
    /// Invalid destination, unreadable authorization file or unavailable HTTP client.
    pub fn with_registry(
        mut self,
        settings: &crate::config::ArtifactRegistry,
    ) -> Result<Self, Error> {
        self.1 = Some(Arc::new(registry::Publisher::open(settings)?));
        Ok(self)
    }

    /// Reserves capacity or returns the existing verified receipt for this exact identity.
    /// The caller must separately authorize the holder before intake and before commit.
    ///
    /// # Errors
    /// Invalid identity, conflicting replay, busy/full storage or filesystem failure.
    pub async fn begin(&self, device: Uuid, offer: Offer) -> Result<Intake, Error> {
        if device.is_nil() || !offer.valid() {
            return Err(Error::Invalid);
        }
        let permit = self
            .0
            .turn
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Busy)?;
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.reserve(device, offer, permit))
            .await
            .map_err(|_| Error::Invalid)?
    }

    /// Checks the private receipt of one upload after the authenticated device's request turn.
    /// The report path only checks retention metadata. Archive replay rehashes the bytes.
    pub async fn confirms_retention(&self, device: Uuid, job: Uuid, attempt: u32) -> bool {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            let path = store.0.root.join(model::attempt_directory(job, attempt));
            disk::receipt(&path).is_ok_and(|receipt| receipt.device == device)
                && disk::sync(&store.0.root).is_ok()
        })
        .await
        .unwrap_or(false)
    }

    fn reserve(
        &self,
        device: Uuid,
        offer: Offer,
        permit: OwnedSemaphorePermit,
    ) -> Result<Intake, Error> {
        let folder = &self.0;
        let destination = folder.root.join(model::directory(&offer));
        if destination.try_exists()? {
            let receipt = disk::receipt(&destination)?;
            if receipt.device != device || receipt.artifact != offer {
                return Err(Error::Conflict);
            }
            disk::verified_file(&destination.join("archive.oci"), &offer)?;
            disk::sync(&folder.root)?;
            return Ok(Intake::Retained(receipt));
        }
        disk::recover(&folder.root)?;
        let (used, entries) = disk::inventory(&folder.root)?;
        if entries >= MAX_RETAINED
            || used
                .checked_add(offer.bytes)
                .is_none_or(|total| total > folder.quota)
        {
            return Err(Error::Capacity);
        }
        disk::capacity(&folder.root, folder.reserve, offer.bytes)?;
        let stage = tempfile::Builder::new()
            .prefix(".incoming-")
            .rand_bytes(16)
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(&folder.root)?;
        let receipt = Receipt {
            version: 1,
            id: Uuid::now_v7(),
            device,
            artifact: offer,
        };
        Ok(Intake::Upload(Upload::new(
            self.clone(),
            permit,
            stage,
            receipt,
        )))
    }
}
