//! Error texts at the scheduler's I/O boundary hold no SQL, URL, header or secret (LOG-007).

use std::fmt::Display;

use crate::{
    fleet::{checks, events},
    ids::DeviceId,
};

#[derive(Debug, thiserror::Error)]
pub(super) enum Fault {
    #[error(transparent)]
    Checks(#[from] checks::Error),
    #[error(transparent)]
    Events(#[from] events::Error),
}

impl From<sqlx::Error> for Fault {
    fn from(error: sqlx::Error) -> Self {
        Self::Checks(error.into())
    }
}

pub(super) fn warned(device: Option<DeviceId>, what: &'static str, error: &dyn Display) {
    tracing::warn!(domain = "checks", device_id = ?device, %error, "{what}");
}

pub(super) fn logged<T, E: Display>(
    device: Option<DeviceId>,
    what: &'static str,
    result: Result<T, E>,
) -> Option<T> {
    result.inspect_err(|error| warned(device, what, error)).ok()
}
