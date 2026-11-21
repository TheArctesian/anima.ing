//! The macOS token store: one generic password in the login Keychain (MET-013).
//!
//! Compiled only on macOS. Each read is bounded ([`crate::keychain_wait`]): a read that macOS
//! holds for the person is [`TokenStoreError::KeychainWaits`].

use keyring::Entry;
use zeroize::Zeroizing;

use super::TokenStoreError;
use crate::config::home::Home;
use crate::keychain_wait::{self, Reads, Unread};

/// What one read of the item gives; the error as text, so that it crosses the thread.
type Answer = Result<Option<Zeroizing<Vec<u8>>>, String>;

/// The reads of the token's item: one in flight at a time (R-NEW-keychainwait).
static READS: Reads<Answer> = Reads::new();

const SERVICE: &str = "ing.anima.daemon";
const ACCOUNT: &str = "daemon-token";

pub struct Backend;

impl Backend {
    pub const fn open(_home: &Home) -> Self {
        Self
    }

    #[expect(
        clippy::unused_self,
        reason = "same signature as the file backend on other systems"
    )]
    pub fn read(&self) -> Result<Option<Zeroizing<Vec<u8>>>, TokenStoreError> {
        match READS.read(keychain_wait::DEADLINE, read_now) {
            Ok(answer) => answer.map_err(TokenStoreError::Keychain),
            Err(Unread::Waits) => Err(TokenStoreError::KeychainWaits),
            Err(Unread::Lost) => Err(TokenStoreError::Keychain(Unread::Lost.to_string())),
        }
    }

    /// Writes the record. A `fresh` write (enrolment) deletes the item first, so the new item
    /// gets a fresh access list rather than inheriting one a stale program was added to.
    #[expect(
        clippy::unused_self,
        reason = "same signature as the file backend on other systems"
    )]
    pub fn write(&self, bytes: &[u8], fresh: bool) -> Result<(), TokenStoreError> {
        if fresh {
            match entry()?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => {}
                Err(error) => return Err(TokenStoreError::Keychain(error.to_string())),
            }
        }
        let text = std::str::from_utf8(bytes).map_err(|_| TokenStoreError::Corrupt)?;
        entry()?
            .set_password(text)
            .map_err(|error| TokenStoreError::Keychain(error.to_string()))
    }

    /// Deletes the item; an item that is not there is not an error.
    #[expect(
        clippy::unused_self,
        reason = "same signature as the file backend on other systems"
    )]
    pub fn delete(&self) -> Result<(), TokenStoreError> {
        match entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(TokenStoreError::Keychain(error.to_string())),
        }
    }
}

/// One read of the item, on the thread of [`READS`]: it can wait for the person for ever.
fn read_now() -> Answer {
    let entry = Entry::new(SERVICE, ACCOUNT).map_err(|error| error.to_string())?;
    match entry.get_password() {
        Ok(text) => Ok(Some(Zeroizing::new(text.into_bytes()))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn entry() -> Result<Entry, TokenStoreError> {
    Entry::new(SERVICE, ACCOUNT).map_err(|error| TokenStoreError::Keychain(error.to_string()))
}
