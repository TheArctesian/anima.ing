//! The files tunnel (RA-032, K4 Section 9): only the `sftp` subsystem, over the hub's own SSH
//! link of a live SSH session (P2-003, SEC-007).
//!
//! No log line, error or event holds a host, an address, a path, the device's text, file
//! content or a token (LOG-007); the audit entry holds the path and the size, never content.

mod device;
mod graphql;
mod opening;
mod protocol;
mod relay;
mod requests;
mod route;
#[cfg(test)]
mod tests;
mod tickets;
mod trail;
mod transfer;

pub use graphql::Mutation as FilesMutation;
pub use route::files_router;
pub(super) use tickets::Tickets;
