//! `anima-daemon enrol`: trade a code for a token and keep both (MET-012, MET-013, K8 Section 3).

use chrono::{DateTime, Utc};
use daemon_core::wire::{EnrolmentCode, RefusalCode, is_device_name};

use crate::config::Config;
use crate::config::home::{Home, HomeError};
use crate::config::origin::Origin;
use crate::hub::HubClient;
use crate::hub::answer::{Answer, Cause};
use crate::token_store::{Credential, TokenStore, TokenStoreError};

/// What `enrol` prints for every refusal: the hub says nothing about which codes exist, and
/// neither does the daemon (K8 Section 3).
pub const REFUSED: &str = "The hub refused this enrolment code. Make a new code on the device's page and run the command again.";

#[derive(Debug, thiserror::Error)]
pub enum EnrolError {
    #[error("{REFUSED}")]
    Refused,
    #[error("{0}; if the hub may have received the code, make a new one")]
    Failed(Cause),
    #[error("the code was not sent; the daemon's folder cannot be used: {0}")]
    Folder(HomeError),
    /// A read of the stored token waits for the person in the Keychain (R-322): a save would
    /// wait too, so the code is kept for a later try.
    #[error(
        "the code was not sent: allow the daemon in the Keychain (click Always Allow in the window of macOS), then enrol again with the same code"
    )]
    KeychainWaits,
    /// The save of the new token waits for the person in the Keychain (R-322). The hub spent
    /// the code, and the token it gave is not kept.
    #[error(
        "enrolled, but the Keychain did not store the token in time, and the code is spent: allow the daemon in the Keychain (click Always Allow in the window of macOS), then make a new code on the device's page and enrol again"
    )]
    TokenWaits,
    #[error("enrolled, but the token could not be kept: {0}")]
    Token(TokenStoreError),
    #[error("enrolled, but the configuration could not be kept: {0}")]
    Home(#[from] HomeError),
}

/// The places an enrolment writes to and the hub it speaks to.
pub struct Enrolment<'a> {
    pub hub: &'a HubClient,
    /// The origin stored in the configuration; the client may have been built from another URL
    /// only in tests.
    pub origin: Origin,
    pub home: &'a Home,
    pub store: &'a TokenStore,
    /// The time of the enrolment, which caps the expiry the hub names.
    pub now: DateTime<Utc>,
}

impl Enrolment<'_> {
    /// Checks the folder and the token store, spends `code` at the hub, stores the token with
    /// its expiry, then the configuration. Returns the name of the device. A folder that is
    /// refused, or a Keychain that waits for the person, is found before the hub sees the
    /// code, so the code is not spent for a token that has nowhere safe to go.
    ///
    /// # Errors
    /// [`EnrolError::Folder`] when the folder is refused and [`EnrolError::KeychainWaits`]
    /// when a read of the store waits (the hub is not called); [`EnrolError::Refused`] for
    /// `ENROLMENT_REFUSED` and [`EnrolError::Failed`] for any other failure of the request. A
    /// failure to store after a good answer is reported as such ([`EnrolError::TokenWaits`]
    /// when the save waits): the code is spent then (MET-012).
    pub async fn spend(self, code: EnrolmentCode) -> Result<String, EnrolError> {
        self.home.ensure().map_err(EnrolError::Folder)?;
        ready(&self.store.load())?;
        let enrolled = match self.hub.enrol(code).await {
            Answer::Accepted(enrolled) => enrolled,
            Answer::Refused(RefusalCode::EnrolmentRefused) => return Err(EnrolError::Refused),
            Answer::Refused(other) => {
                return Err(EnrolError::Failed(Cause::Status(other.status())));
            }
            Answer::Failed(cause) => return Err(EnrolError::Failed(cause)),
        };
        // The hub's text goes to the operator's terminal; every name the hub holds passes.
        if !is_device_name(&enrolled.device_name) {
            return Err(EnrolError::Failed(Cause::Unreadable));
        }
        let credential = Credential::received(
            enrolled.token,
            enrolled.expires_at,
            self.origin.clone(),
            self.now,
        );
        self.store.save_new(&credential).map_err(not_kept)?;
        self.home.write_config(&Config {
            hub: self.origin,
            device: enrolled.device,
            device_name: enrolled.device_name.clone(),
        })?;
        Ok(enrolled.device_name)
    }
}

/// Whether the token store answers before the code is spent: only a read that waits for the
/// person in the Keychain stops the enrolment. Any other answer, a damaged record too, is
/// replaced by the new token.
fn ready<T>(loaded: &Result<T, TokenStoreError>) -> Result<(), EnrolError> {
    if matches!(loaded, Err(TokenStoreError::KeychainWaits)) {
        return Err(EnrolError::KeychainWaits);
    }
    Ok(())
}

/// The error of a save of the new token, after the hub spent the code.
fn not_kept(error: TokenStoreError) -> EnrolError {
    match error {
        TokenStoreError::KeychainWaits => EnrolError::TokenWaits,
        other => EnrolError::Token(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn met_012_a_keychain_that_waits_stops_the_enrolment_before_the_code_is_sent() {
        let waits = ready::<()>(&Err(TokenStoreError::KeychainWaits));
        assert!(matches!(waits, Err(EnrolError::KeychainWaits)));
        let text = EnrolError::KeychainWaits.to_string();
        assert!(text.starts_with("the code was not sent"));
        assert!(text.contains("allow the daemon in the Keychain"));
        assert!(text.contains("enrol again"));
        assert!(ready(&Ok(None::<()>)).is_ok(), "no token yet");
        assert!(
            ready::<()>(&Err(TokenStoreError::Corrupt)).is_ok(),
            "a damaged record is replaced"
        );
    }

    #[test]
    fn met_012_a_save_that_waits_fails_with_the_steps_to_enrol_again() {
        let waits = not_kept(TokenStoreError::KeychainWaits);
        assert!(matches!(waits, EnrolError::TokenWaits));
        let text = waits.to_string();
        assert!(text.contains("the code is spent"));
        assert!(text.contains("allow the daemon in the Keychain"));
        assert!(text.contains("make a new code"));
        assert!(matches!(
            not_kept(TokenStoreError::Corrupt),
            EnrolError::Token(TokenStoreError::Corrupt)
        ));
    }
}
