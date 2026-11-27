//! Managed Code commands: person-authorized queue and device-authenticated delivery.
pub(in crate::fleet) mod browser_sessions;
mod delivery;
pub mod graphql;
pub(in crate::fleet) mod previews;
pub mod provider_login;
mod store;
pub(in crate::fleet) use delivery::poll;
#[cfg(unix)]
pub(super) use store::close_for_handoff;
#[cfg(unix)]
pub(super) use store::enqueue as enqueue_continuation;

#[derive(Debug, thiserror::Error)]
pub(in crate::fleet) enum Error {
    #[error("{0}")]
    Refused(&'static str),
    #[error("Managed Code storage is unavailable.")]
    Database(#[from] sqlx::Error),
    #[error("Managed Code numeric identity is out of range.")]
    Integer(#[from] std::num::TryFromIntError),
    #[error("Managed Code data is invalid.")]
    Json(#[from] serde_json::Error),
}
type Result<T> = std::result::Result<T, Error>;
