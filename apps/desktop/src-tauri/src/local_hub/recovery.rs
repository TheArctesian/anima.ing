//! F-9: each account replacement needs its own native-dialog confirmation, which web-view script cannot answer.
//! Pure: the dialog is a parameter.

use std::time::{Duration, Instant};

use super::LocalHubError;

pub const CONFIRMATION_TITLE: &str = "Recover the account";
pub const CONFIRMATION: &str = "This sets a new password and a new authenticator for the \
operator account of the hub on this computer. Every sign-in to the hub ends, on every device.\n\n\
Continue only if you asked for this yourself.";

/// So script cannot raise the dialog again and again until a person clicks it by mistake.
pub const PAUSE_AFTER_CANCEL: Duration = Duration::from_mins(1);

#[derive(Debug, Default)]
pub struct Cancellations {
    last: Option<Instant>,
}

pub fn may_create(
    has_account: bool,
    cancellations: &mut Cancellations,
    now: Instant,
    confirm: impl FnOnce() -> bool,
) -> Result<(), LocalHubError> {
    if !has_account {
        return Ok(());
    }
    let paused = cancellations
        .last
        .is_some_and(|last| now.saturating_duration_since(last) < PAUSE_AFTER_CANCEL);
    if paused {
        return Err(LocalHubError::Busy);
    }
    if !confirm() {
        cancellations.last = Some(now);
        return Err(LocalHubError::Cancelled);
    }
    Ok(())
}
