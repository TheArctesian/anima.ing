//! Human-authorized workspace lifecycle; the execution owner remains the admission authority.
mod delivery;
pub mod graphql;
mod private_checkout;
mod store;
pub(in crate::fleet) use delivery::poll;
#[cfg(unix)]
pub(super) use store::enqueue as enqueue_continuation;

#[derive(Debug, thiserror::Error)]
pub(in crate::fleet) enum Error {
    #[error("{0}")]
    Refused(&'static str),
    #[error("Workspace lifecycle storage is unavailable.")]
    Database(#[from] sqlx::Error),
    #[error("Workspace lifecycle data is invalid.")]
    Json(#[from] serde_json::Error),
}
type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests;
