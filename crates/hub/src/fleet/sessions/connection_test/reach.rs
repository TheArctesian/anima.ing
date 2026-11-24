//! Reaching an RDP or VNC port through the hub's checked connect, the one SSH uses (R-032).
//! I/O edge; nothing here logs.

use std::{future::Future, time::Duration};

use crate::fleet::{
    ssh::{self, SshError, Target},
    targets::Reached,
};

/// Holds no host name or remote text (LOG-007).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(in crate::fleet::sessions) enum ReadError {
    #[error("the host name does not resolve")]
    Unresolved,
    #[error("the host resolves to an address sessions may not reach")]
    RefusedTarget,
    #[error("no connection to the host's port")]
    Unreachable,
    #[error("the host did not finish in time")]
    Timeout,
    #[error("the host did not answer as the protocol does")]
    Unexpected,
    #[error("the host did not complete a TLS handshake")]
    Tls,
}

/// # Errors
/// [`ReadError::Unresolved`], [`ReadError::RefusedTarget`] before any connection, or
/// [`ReadError::Unreachable`].
pub(in crate::fleet::sessions) async fn reach(target: Target<'_>) -> Result<Reached, ReadError> {
    ssh::connect(target).await.map_err(|error| match error {
        SshError::Unresolved => ReadError::Unresolved,
        SshError::RefusedTarget => ReadError::RefusedTarget,
        _ => ReadError::Unreachable,
    })
}

/// # Errors
/// As `read`, or [`ReadError::Timeout`].
pub(in crate::fleet::sessions) async fn within<T>(
    limit: Duration,
    read: impl Future<Output = Result<T, ReadError>>,
) -> Result<T, ReadError> {
    tokio::time::timeout(limit, read)
        .await
        .map_err(|_| ReadError::Timeout)?
}
