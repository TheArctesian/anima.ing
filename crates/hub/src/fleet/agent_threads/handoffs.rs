//! Owner-bound handoff state. A finished or expired transfer can no longer deliver bytes.
#[cfg(unix)]
mod activation;
#[cfg(unix)]
pub(crate) use activation::activated;
#[cfg(unix)]
mod begin;
#[cfg(unix)]
pub(super) mod continuation;
#[cfg(unix)]
pub(super) mod graphql;
#[cfg(unix)]
pub(crate) mod operator;
#[cfg(unix)]
mod placement;
#[cfg(unix)]
mod retirement;
#[cfg(unix)]
mod source;
#[cfg(unix)]
pub(crate) use retirement::report as report_retirement;
#[cfg(unix)]
mod store;
#[cfg(unix)]
mod withdrawal;
#[cfg(unix)]
pub(crate) use store::{confirmed, issued, prepare, read, restored, staged};
#[cfg(unix)]
pub(crate) use withdrawal::report as report_withdrawal;

#[cfg(unix)]
use crate::ids::DeviceId;
#[cfg(unix)]
use agent_threads::managed::workspace::{
    Source as Checkout,
    handoff::{Binding, Import, Offer, Receipt, Source, valid_generation},
};
#[cfg(unix)]
use chrono::{DateTime, Utc};
#[cfg(unix)]
use sqlx::types::Json;
use uuid::Uuid;

#[cfg(unix)]
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("{0}")]
    Refused(&'static str),
    #[error("Workspace transfer storage is unavailable.")]
    Database(#[from] sqlx::Error),
}

#[cfg(unix)]
type Result<T> = std::result::Result<T, Error>;

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MoveRequest {
    transfer: Uuid,
    workspace: Uuid,
    source_device: DeviceId,
    generation: u64,
    storage_bytes: u64,
    continue_session: bool,
}

#[cfg(unix)]
impl MoveRequest {
    fn fits(self) -> bool {
        !self.transfer.is_nil()
            && !self.workspace.is_nil()
            && !Uuid::from(self.source_device).is_nil()
            && valid_generation(self.generation)
            && valid_generation(self.generation + 1)
            && (1 << 30..=64 << 30).contains(&self.storage_bytes)
    }
}

#[cfg(unix)]
#[derive(Clone, Copy)]
struct Start {
    request: MoveRequest,
    destination_device: DeviceId,
    headroom: Option<source::Limits>,
}

#[cfg(unix)]
impl Start {
    fn fits(self) -> bool {
        self.request.fits()
            && !Uuid::from(self.destination_device).is_nil()
            && self.request.source_device != self.destination_device
    }
}

#[cfg(unix)]
#[derive(sqlx::FromRow)]
pub(crate) struct Transfer {
    #[sqlx(rename = "transfer")]
    id: Uuid,
    workspace: Uuid,
    principal: Uuid,
    source_device: DeviceId,
    destination_device: DeviceId,
    generation: i64,
    storage_bytes: i64,
    continue_session: bool,
    checkout: Option<Json<Checkout>>,
    pub(crate) checkpoint: Option<Json<Receipt>>,
    pub(crate) confirmed: bool,
    restored_at: Option<DateTime<Utc>>,
    activation_offered_at: Option<DateTime<Utc>>,
    pub(crate) expires_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
    withdraw_requested_at: Option<DateTime<Utc>>,
    destination_discarded_at: Option<DateTime<Utc>>,
    retire_requested_at: Option<DateTime<Utc>>,
    retired_at: Option<DateTime<Utc>>,
}

#[cfg(unix)]
impl Transfer {
    pub(crate) fn live(&self, now: DateTime<Utc>) -> bool {
        self.finished_at.is_none() && self.withdraw_requested_at.is_none() && now < self.expires_at
    }

    fn source(&self) -> Option<Source> {
        Some(Source {
            workspace: self.workspace,
            principal: self.principal,
            generation: u64::try_from(self.generation).ok()?,
            binding: Binding {
                transfer: self.id,
                destination: self.destination_device.into(),
            },
        })
    }

    fn checkpoint_import(&self) -> Option<Import> {
        self.import(&self.checkpoint.as_ref()?.0)
    }

    pub(crate) fn import(&self, receipt: &Receipt) -> Option<Import> {
        let result = Import {
            receipt: receipt.clone(),
            source_device: self.source_device.into(),
            destination_device: self.destination_device.into(),
            storage_bytes: u64::try_from(self.storage_bytes).ok()?,
            checkout: self.checkout.as_ref().map(|checkout| checkout.0.clone()),
        };
        (Some(receipt.source) == self.source() && result.fits()).then_some(result)
    }
}

pub(super) async fn reserved(
    db: &mut sqlx::PgConnection,
    workspace: Uuid,
) -> std::result::Result<bool, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM agent_threads.workspace_handoffs
        WHERE workspace=$1 AND finished_at IS NULL)",
    )
    .bind(workspace)
    .fetch_one(db)
    .await
}
