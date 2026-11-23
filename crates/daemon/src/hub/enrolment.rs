//! `anima-daemon enrol`: trade a code for a token and keep both (MET-012, MET-013, K8 Section 3).

use std::time::Duration;

use chrono::{DateTime, Utc};
use daemon_core::wire::{EnrolmentCode, RefusalCode, is_device_name};

use crate::config::Config;
use crate::config::home::{Home, HomeError};
use crate::config::origin::Origin;
use crate::hub::HubClient;
use crate::hub::answer::{Answer, Cause};
use crate::token_store::{Credential, TokenStore, TokenStoreError};

/// One text for every refusal: the daemon tells no more than the hub about which codes exist
/// (K8 Section 3).
pub const REFUSED: &str = "The hub refused this enrolment code. Make a new code on the device's page and run the command again.";

/// A person answers the Keychain window of macOS at the terminal; the running daemon keeps its
/// 5 s (R-322).
pub const KEYCHAIN_DEADLINE: Duration = Duration::from_secs(120);

#[derive(Debug, thiserror::Error)]
pub enum EnrolError {
    #[error("{REFUSED}")]
    Refused,
    #[error("{0}; if the hub may have received the code, make a new one")]
    Failed(Cause),
    #[error("the code was not sent; the daemon's folder cannot be used: {0}")]
    Folder(HomeError),
    #[error(
        "the code was not sent: allow the daemon in the Keychain (click Always Allow in the window of macOS), then enrol again with the same code"
    )]
    KeychainWaits,
    #[error("the code was not sent; the stored token cannot be read: {0}")]
    Unread(TokenStoreError),
    #[error(
        "enrolled, but the Keychain did not store the token in time: the code is spent; make a new one on the device's page, allow the daemon in the Keychain (click Always Allow in the window of macOS), and enrol again"
    )]
    TokenWaits,
    #[error("enrolled, but the token could not be kept: {0}; the code is spent; make a new one")]
    Token(TokenStoreError),
    #[error("enrolled, but the configuration could not be kept: {0}")]
    Home(#[from] HomeError),
}

pub struct Enrolment<'a> {
    pub hub: &'a HubClient,
    /// The stored origin; only tests build the client from another URL.
    pub origin: Origin,
    pub home: &'a Home,
    pub store: &'a TokenStore,
    /// The time of the enrolment, which caps the expiry the hub names.
    pub now: DateTime<Utc>,
}

impl Enrolment<'_> {
    /// The folder and the token store are checked before the hub sees the code, so the code is
    /// not spent for a token that has nowhere safe to go.
    ///
    /// # Errors
    /// [`EnrolError`]; one after a good answer means the code is spent (MET-012).
    pub async fn spend(self, code: EnrolmentCode) -> Result<String, EnrolError> {
        self.home.ensure().map_err(EnrolError::Folder)?;
        let store = self.store.waiting(KEYCHAIN_DEADLINE);
        ready(store.load())?;
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
        store.save_new(&credential).map_err(not_kept)?;
        self.home.write_config(&Config {
            hub: self.origin,
            device: enrolled.device,
            device_name: enrolled.device_name.clone(),
        })?;
        Ok(enrolled.device_name)
    }
}

fn ready<T>(loaded: Result<T, TokenStoreError>) -> Result<(), EnrolError> {
    match loaded {
        Ok(_) | Err(TokenStoreError::Corrupt) => Ok(()),
        Err(TokenStoreError::KeychainWaits) => Err(EnrolError::KeychainWaits),
        Err(error) => Err(EnrolError::Unread(error)),
    }
}

fn not_kept(error: TokenStoreError) -> EnrolError {
    match error {
        TokenStoreError::KeychainWaits => EnrolError::TokenWaits,
        other => EnrolError::Token(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::private_file::PrivateFileError;

    #[test]
    fn met_012_a_token_store_that_waits_or_fails_stops_the_enrolment_before_the_code_is_sent() {
        let waits = ready::<()>(Err(TokenStoreError::KeychainWaits));
        assert!(matches!(waits, Err(EnrolError::KeychainWaits)));
        let text = EnrolError::KeychainWaits.to_string();
        assert!(text.starts_with("the code was not sent"));
        assert!(text.contains("allow the daemon in the Keychain"));
        assert!(text.contains("enrol again with the same code"));
        let unread = ready::<()>(Err(PrivateFileError::Exposed.into()));
        let Err(unread) = unread else {
            panic!("a store that cannot be read lets the code go");
        };
        assert!(unread.to_string().starts_with("the code was not sent"));
        assert!(ready(Ok(None::<()>)).is_ok(), "no token yet");
        assert!(
            ready::<()>(Err(TokenStoreError::Corrupt)).is_ok(),
            "a damaged record is replaced"
        );
    }

    #[test]
    fn met_012_a_save_after_the_code_is_spent_says_to_make_a_new_one() {
        let waits = not_kept(TokenStoreError::KeychainWaits);
        assert!(matches!(waits, EnrolError::TokenWaits));
        let text = waits.to_string();
        assert!(text.contains("the code is spent; make a new one"));
        assert!(text.contains("allow the daemon in the Keychain"));
        let failed = not_kept(TokenStoreError::Corrupt);
        assert!(matches!(
            failed,
            EnrolError::Token(TokenStoreError::Corrupt)
        ));
        assert!(
            failed
                .to_string()
                .contains("the code is spent; make a new one")
        );
    }
}
