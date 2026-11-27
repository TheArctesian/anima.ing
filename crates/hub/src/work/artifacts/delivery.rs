use super::{Error, Intake, Offer, Receipt, Store};
use crate::{
    clock::Clock,
    ids::{DeviceId, JobId},
    work::jobs::{self, ArtifactAccess, ArtifactGuard},
};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::io::AsyncRead;

#[derive(Debug, thiserror::Error)]
pub enum DeliveryError {
    #[error("the device does not hold this source-build attempt")]
    NotHolder,
    #[error("the device is not permitted to deliver work")]
    Off,
    #[error("artifact stream or revalidation cannot be completed now")]
    Unavailable,
    #[error("artifact storage failed: {0}")]
    Storage(#[from] Error),
    #[error("artifact ownership could not be checked: {0}")]
    Jobs(#[from] jobs::Error),
}

/// The authenticated transport supplies the device; the job owner supplies authority.
pub struct Delivery<'a> {
    pub store: &'a Store,
    pub db: &'a PgPool,
    pub clock: Arc<dyn Clock>,
}

/// Staged bytes or an exact replay, bound to the authenticated device.
pub struct Pending {
    device: DeviceId,
    offer: Offer,
    output: Output,
}

enum Output {
    Retained(Receipt),
    Verified(super::Verified),
}

impl Delivery<'_> {
    async fn guard(&self, device: DeviceId, offer: &Offer) -> Result<ArtifactGuard, DeliveryError> {
        let access = jobs::artifact_access(
            self.db,
            device,
            (JobId::from(offer.job), offer.attempt),
            self.clock.as_ref(),
        );
        match tokio::time::timeout(std::time::Duration::from_secs(5), access)
            .await
            .map_err(|_| DeliveryError::Unavailable)??
        {
            ArtifactAccess::Granted(guard) => Ok(guard),
            ArtifactAccess::NotHolder => Err(DeliveryError::NotHolder),
            ArtifactAccess::Off => Err(DeliveryError::Off),
        }
    }

    /// Verifies private output without holding database locks during transfer or rehashing.
    ///
    /// # Errors
    /// Lost or stopped ownership, work disabled, invalid stream or unavailable storage.
    pub async fn prepare(
        &self,
        device: DeviceId,
        offer: Offer,
        input: impl AsyncRead + Unpin,
    ) -> Result<Pending, DeliveryError> {
        if !self
            .guard(device, &offer)
            .await?
            .finish(self.clock.now())
            .await?
        {
            return Err(DeliveryError::NotHolder);
        }
        let intake = self.store.begin(device.into(), offer.clone()).await?;
        let output = match intake {
            Intake::Retained(receipt) => Output::Retained(receipt),
            Intake::Upload(upload) => Output::Verified(upload.receive(input).await?),
        };
        Ok(Pending {
            device,
            offer,
            output,
        })
    }

    /// Rechecks ownership before commit and expiry before acknowledging retention.
    ///
    /// # Errors
    /// Lost or stopped ownership, work disabled or unavailable database or storage.
    pub async fn retain(&self, pending: Pending) -> Result<Receipt, DeliveryError> {
        let guard = self.guard(pending.device, &pending.offer).await?;
        let receipt = match pending.output {
            Output::Retained(receipt) => Some(receipt),
            Output::Verified(verified) => {
                verified
                    .retain_before(self.clock.clone(), guard.valid_until())
                    .await?
            }
        };
        if !guard.finish(self.clock.now()).await? {
            return Err(DeliveryError::NotHolder);
        }
        receipt.ok_or(DeliveryError::NotHolder)
    }
}
