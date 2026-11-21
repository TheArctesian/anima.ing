//! The macOS key store: one generic password in the login Keychain, beside the token's item
//! (K-E5 Section 2.1). Each read is bounded ([`crate::keychain_wait`]).

use keyring::Entry;
use zeroize::Zeroizing;

use super::KeyStoreError;
use crate::config::home::Home;
use crate::keychain_wait::{self, Reads, Unread};

/// What one read of the item gives; the error as its fixed text, so that it crosses the thread.
type Answer = Result<Option<Zeroizing<Vec<u8>>>, String>;

/// The reads of the key cache's item: one in flight at a time (R-NEW-keychainwait).
static READS: Reads<Answer> = Reads::new();

const SERVICE: &str = "ing.anima.daemon";
const ACCOUNT: &str = "key-cache";

pub struct Backend;

impl Backend {
    pub const fn open(_home: &Home) -> Self {
        Self
    }

    #[expect(
        clippy::unused_self,
        reason = "same signature as the file backend on other systems"
    )]
    pub fn read(&self, limit: usize) -> Result<Option<Zeroizing<Vec<u8>>>, KeyStoreError> {
        let bytes = match READS.read(keychain_wait::DEADLINE, read_now) {
            Ok(answer) => answer.map_err(KeyStoreError::Keychain)?,
            Err(Unread::Waits) => return Err(KeyStoreError::KeychainWaits),
            Err(Unread::Lost) => return Err(KeyStoreError::Keychain(Unread::Lost.to_string())),
        };
        Ok(bytes.filter(|bytes| bytes.len() <= limit))
    }

    #[expect(
        clippy::unused_self,
        reason = "same signature as the file backend on other systems"
    )]
    pub fn write(&self, bytes: &[u8]) -> Result<(), KeyStoreError> {
        // The record is JSON, so always UTF-8; anything else is never written.
        let text = std::str::from_utf8(bytes)
            .map_err(|_| KeyStoreError::Keychain("the record is not text".to_owned()))?;
        entry()?.set_password(text).map_err(|error| fault(&error))
    }

    #[expect(
        clippy::unused_self,
        reason = "same signature as the file backend on other systems"
    )]
    pub fn delete(&self) -> Result<(), KeyStoreError> {
        match entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(error) => Err(fault(&error)),
        }
    }
}

/// One read of the item, on the thread of [`READS`]: it can wait for the person for ever.
fn read_now() -> Answer {
    let entry = Entry::new(SERVICE, ACCOUNT).map_err(|error| text(&error))?;
    match entry.get_password() {
        Ok(text) => Ok(Some(Zeroizing::new(text.into_bytes()))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(text(&error)),
    }
}

fn entry() -> Result<Entry, KeyStoreError> {
    Entry::new(SERVICE, ACCOUNT).map_err(|error| fault(&error))
}

fn fault(error: &keyring::Error) -> KeyStoreError {
    KeyStoreError::Keychain(text(error))
}

/// The Keychain's error as text. An item that is not UTF-8 is named by a fixed sentence: that
/// error carries the item's bytes, which may be a value (LOG-007).
fn text(error: &keyring::Error) -> String {
    match error {
        keyring::Error::BadEncoding(_) => "the item is not text".to_owned(),
        other => other.to_string(),
    }
}
