//! Private staging for project checkpoints. Database authorization lives with handoffs.
mod disk;
mod stage;

use super::{Error, Store};
use agent_threads::managed::workspace::handoff::Import;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::{io::AsyncRead, sync::OwnedSemaphorePermit};
use uuid::Uuid;

pub(super) const DIRECTORY: &str = "workspace-checkpoints";
pub(super) use disk::{inventory, recover};
pub(crate) use stage::Pending;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    import: Import,
    expires_at: DateTime<Utc>,
}

impl Stored {
    fn transfer(&self) -> Uuid {
        self.import.receipt.source.binding.transfer
    }
}

pub(crate) struct Open {
    pub(crate) file: tokio::fs::File,
    pub(crate) _permit: OwnedSemaphorePermit,
}

impl Store {
    pub(crate) async fn receive_checkpoint(
        &self,
        import: Import,
        (now, expires_at): (DateTime<Utc>, DateTime<Utc>),
        input: impl AsyncRead + Unpin,
    ) -> Result<Pending, Error> {
        if !import.fits() || now >= expires_at {
            return Err(Error::Invalid);
        }
        let permit = self
            .0
            .turn
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Busy)?;
        let store = self.clone();
        let stored = Stored { import, expires_at };
        let staged =
            tokio::task::spawn_blocking(move || stage::reserve(store, stored, permit, now))
                .await
                .map_err(|_| Error::Invalid)??;
        staged.receive(input).await
    }

    pub(crate) async fn open_checkpoint(&self, import: Import) -> Result<Open, Error> {
        let permit = self
            .0
            .readers
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Busy)?;
        let node = self.clone();
        let file = tokio::task::spawn_blocking(move || disk::open(&node, &import))
            .await
            .map_err(|_| Error::Invalid)??;
        Ok(Open {
            file: tokio::fs::File::from_std(file),
            _permit: permit,
        })
    }

    /// Expiry revokes access in the database first. This removes the unneeded disk bytes.
    ///
    /// # Errors
    /// Another writer owns the shared store or filesystem validation failed.
    pub async fn reap_checkpoints(&self, now: DateTime<Utc>) -> Result<(), Error> {
        let permit = self
            .0
            .turn
            .clone()
            .try_acquire_owned()
            .map_err(|_| Error::Busy)?;
        let path = self.0.root.join(DIRECTORY);
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            disk::reap(&path, now)
        })
        .await
        .map_err(|_| Error::Invalid)?
    }
}
