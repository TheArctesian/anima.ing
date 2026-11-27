mod inventory;
pub mod lease;
mod sandbox;
pub use inventory::list as inventory;
mod cleanup;
pub use cleanup::release;
mod artifact;
mod launch;
mod lifecycle;
mod output;
mod verification;
pub use lifecycle::{cancel, reconcile, status};
pub use output::read as logs;
pub(super) mod store;
pub mod worker;
pub use launch::{Choice, start};
pub use store::{default_seconds, reserved_bytes};
pub(super) mod export;
pub use export::stream as export;

pub(super) async fn cancel_if_present(id: uuid::Uuid) -> super::Result<()> {
    if store::base(id).try_exists()? {
        cancel(id).await?;
    }
    Ok(())
}

pub(super) async fn release_if_present(id: uuid::Uuid) -> super::Result<()> {
    if store::base(id).try_exists()? {
        release(id, true).await?;
    }
    Ok(())
}
