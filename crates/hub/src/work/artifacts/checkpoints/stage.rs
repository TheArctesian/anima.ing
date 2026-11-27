use super::{DIRECTORY, Error, Store, Stored, disk};
use crate::{
    clock::Clock,
    work::artifacts::{MAX_RETAINED, disk as storage, transfer},
};
use chrono::{DateTime, Utc};
use std::{
    fs::Permissions, os::unix::fs::PermissionsExt as _, path::PathBuf, sync::Arc, time::Duration,
};
use tempfile::TempDir;
use tokio::{io::AsyncRead, sync::OwnedSemaphorePermit};

struct Slot {
    directory: Option<TempDir>,
    destination: PathBuf,
    stored: Stored,
    _permit: OwnedSemaphorePermit,
    _store: Store,
}

pub(crate) struct Pending(Option<Slot>);

impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(slot) = self.0.take() {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn_blocking(move || drop(slot));
            } else {
                drop(slot);
            }
        }
    }
}

pub(super) fn reserve(
    store: Store,
    stored: Stored,
    permit: OwnedSemaphorePermit,
    now: DateTime<Utc>,
) -> Result<Pending, Error> {
    let root = store.0.root.join(DIRECTORY);
    disk::create(&root)?;
    disk::reap(&root, now)?;
    let destination = root.join(stored.transfer().to_string());
    let directory = if destination.try_exists()? {
        if disk::stored(&destination)? != stored {
            return Err(Error::Conflict);
        }
        storage::verified_bytes(
            &destination.join("project.tar"),
            stored.import.receipt.bytes,
            &stored.import.receipt.digest,
        )?;
        None
    } else {
        let bytes = stored.import.receipt.bytes;
        let (used, entries) = storage::inventory(&store.0.root)?;
        if entries >= MAX_RETAINED
            || used
                .checked_add(bytes)
                .is_none_or(|total| total > store.0.quota)
        {
            return Err(Error::Capacity);
        }
        storage::capacity(&root, store.0.reserve, bytes)?;
        Some(
            tempfile::Builder::new()
                .prefix(".incoming-")
                .rand_bytes(16)
                .permissions(Permissions::from_mode(0o700))
                .tempdir_in(&root)?,
        )
    };
    Ok(Pending(Some(Slot {
        directory,
        destination,
        stored,
        _permit: permit,
        _store: store,
    })))
}

impl Pending {
    pub(super) async fn receive(self, input: impl AsyncRead + Unpin) -> Result<Self, Error> {
        tokio::time::timeout(Duration::from_mins(30), self.receive_inner(input))
            .await
            .map_err(|_| Error::Invalid)?
    }

    async fn receive_inner(self, input: impl AsyncRead + Unpin) -> Result<Self, Error> {
        let slot = self.0.as_ref().ok_or(Error::Invalid)?;
        if let Some(dir) = &slot.directory {
            let receipt = &slot.stored.import.receipt;
            transfer::receive_exact(
                &dir.path().join("project.tar"),
                (receipt.bytes, &receipt.digest),
                input,
            )
            .await?;
            let path = dir.path().to_owned();
            let stored = slot.stored.clone();
            return tokio::task::spawn_blocking(move || {
                storage::write_metadata(&path, &stored)?;
                Ok(self)
            })
            .await
            .map_err(|_| Error::Invalid)?;
        }
        Ok(self)
    }

    /// The coordinator holds authorization through the rename and directory sync.
    pub(crate) async fn retain(
        mut self,
        clock: Arc<dyn Clock>,
        authorized_until: DateTime<Utc>,
    ) -> Result<bool, Error> {
        let slot = self.0.take().ok_or(Error::Invalid)?;
        tokio::task::spawn_blocking(move || retain(&slot, clock.as_ref(), authorized_until))
            .await
            .map_err(|_| Error::Invalid)?
    }
}

fn retain(slot: &Slot, clock: &dyn Clock, authorized_until: DateTime<Utc>) -> Result<bool, Error> {
    let deadline = slot.stored.expires_at.min(authorized_until);
    if clock.now() >= deadline {
        return Ok(false);
    }
    let parent = slot.destination.parent().ok_or(Error::Private)?;
    let Some(dir) = &slot.directory else {
        storage::sync(parent)?;
        return Ok(clock.now() < deadline);
    };
    std::fs::rename(dir.path(), &slot.destination)?;
    let sync = storage::sync(parent);
    if sync.is_err() || clock.now() >= deadline {
        std::fs::rename(&slot.destination, dir.path())?;
        storage::sync(parent)?;
        sync?;
        return Ok(false);
    }
    Ok(true)
}
