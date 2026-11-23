//! The daemon's log lines: what happened, by code or cause, never a token, a code or a body (LOG-007).

pub mod log_file;

use daemon_core::agent_usage::UsageState;
use daemon_core::wire::RefusalCode;

use crate::config::agent_usage::SettingsError;
use crate::hub::answer::Cause;
use crate::token_store::TokenStoreError;

pub fn batch_dropped(code: RefusalCode, points: usize) {
    tracing::warn!(
        code = code.as_str(),
        points,
        "the hub refused a batch; dropped it"
    );
}

pub fn push_failed(cause: Cause) {
    tracing::warn!(%cause, "push failed; keeping the points");
}

pub fn hub_cannot_store() {
    tracing::warn!("the hub cannot store now; keeping the points");
}

pub fn hub_back() {
    tracing::info!("the hub answers again");
}

pub fn token_refused(code: RefusalCode) {
    tracing::error!(
        code = code.as_str(),
        "the hub refused the daemon's token; pushing stopped, sampling goes on"
    );
}

pub fn token_accepted_again() {
    tracing::info!("the hub accepts the token again");
}

pub fn token_store_changed() {
    tracing::info!("the token store changed; pushing again");
}

pub fn renewed() {
    tracing::info!("the daemon token was renewed");
}

pub fn renewal_not_stored(error: &TokenStoreError) {
    tracing::error!(%error, "the new token could not be stored; keeping the current one");
}

/// Once, when the save of a renewed token first waits for the person in the Keychain (R-322).
pub fn renewal_waits() {
    tracing::warn!(
        "this computer's daemon needs you to allow it in the Keychain to store its renewed token: click Always Allow in the window of macOS; it keeps the current token, which the hub accepts until the new one is used, and tries again every 30 s, after 10 minutes every 5 minutes"
    );
}

pub fn renewal_unsure(error: &TokenStoreError) {
    tracing::error!(%error, "the renewed token could not be stored; keeping it and the current one, and trying again");
}

/// The delete of a token that the hub refused waits for the person in the Keychain (R-322).
pub fn forget_waits() {
    tracing::warn!(
        "the refused token could not be deleted: this computer's daemon needs you to allow it in the Keychain; click Always Allow in the window of macOS"
    );
}

pub fn renewal_refused(code: RefusalCode) {
    tracing::warn!(code = code.as_str(), "renewal refused");
}

pub fn renewal_failed(cause: Cause) {
    tracing::warn!(%cause, "renewal failed; trying again in an hour");
}

pub fn running(device: uuid::Uuid) {
    tracing::info!(%device, "the daemon is running");
}

pub fn agent_usage(state: UsageState) {
    tracing::info!(state = state.as_str(), "agent usage counts");
}

/// Once at start: the settings of the agent usage cannot be read, so it is off (AIS-008).
pub fn agent_usage_settings_refused(error: &SettingsError) {
    tracing::error!(%error, "agent usage counts are off until the settings file is corrected");
}

pub fn agent_usage_read_failed() {
    tracing::error!("reading the agent session files stopped: its task panicked");
}

pub fn stopping() {
    tracing::info!("the daemon is stopping");
}

pub fn sampling_ended(ended: &Result<(), tokio::task::JoinError>) {
    if ended.is_err() {
        sampling_panicked();
    } else {
        sampling_returned();
    }
}

fn sampling_panicked() {
    tracing::error!("sampling stopped: its task panicked");
}

fn sampling_returned() {
    tracing::error!("sampling stopped before the daemon was told to stop");
}

/// An old link origin that did not move; the move is asked for again each minute (R-317).
#[cfg(unix)]
pub fn old_origin_waits(socket: &std::path::Path) {
    tracing::warn!(
        socket = %socket.display(),
        "enrolled with an old link origin: sending through this socket of anima-link, only to the link of this user that passes the check, until the move to it"
    );
}

#[cfg(unix)]
pub fn moved() {
    tracing::info!("the old link origin moved to the socket of anima-link");
}

#[cfg(unix)]
pub fn move_failed(error: &crate::runtime::old_origin::MoveError) {
    tracing::error!(%error, "the old link origin could not move; trying again at the next start");
}

pub fn log_file_refused(error: &std::io::Error) {
    tracing::error!(%error, "the log file could not be opened; logging to standard error");
}

/// Once, when the first read of the token waits for the Keychain (R-322).
pub fn keychain_waits() {
    tracing::warn!(
        "this computer's daemon needs you to allow it in the Keychain: click Always Allow in the window of macOS; it reads the token again every 30 s, after 10 minutes every 5 minutes"
    );
}

pub fn keychain_answered() {
    tracing::info!("the Keychain answered the read of the token");
}

pub fn keychain_mark_refused(error: &std::io::Error) {
    tracing::warn!(%error, "the mark of the wait for the Keychain could not be changed");
}

pub fn run_ended(error: &dyn std::error::Error) {
    tracing::error!(%error, "the daemon stopped on an error");
}
