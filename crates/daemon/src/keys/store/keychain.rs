//! The macOS key store: one generic password in the login Keychain (K-E5 Section 2.1). Each read,
//! write and delete is bounded ([`crate::keychain_wait`]).

use keyring::Entry;
use zeroize::Zeroizing;

use super::KeyStoreError;
use crate::config::home::Home;
use crate::keychain_wait::{self, Calls, Read, Unanswered, Written};

/// The calls of the key cache's item: one in flight at a time (R-322).
static CALLS: Calls = Calls::new();

const SERVICE: &str = "ing.anima.daemon";
const ACCOUNT: &str = "key-cache";

#[derive(Clone)]
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
        let bytes = CALLS
            .read(keychain_wait::DEADLINE, read_now)
            .map_err(unanswered)?
            .map_err(KeyStoreError::Keychain)?;
        Ok(bytes.filter(|bytes| bytes.len() <= limit))
    }

    #[expect(
        clippy::unused_self,
        reason = "same signature as the file backend on other systems"
    )]
    pub fn write(&self, bytes: &[u8]) -> Result<(), KeyStoreError> {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| KeyStoreError::Keychain("the record is not text".to_owned()))?;
        let text = Zeroizing::new(text.to_owned());
        CALLS
            .write(keychain_wait::DEADLINE, move || write_now(&text))
            .map_err(unanswered)?
            .map_err(KeyStoreError::Keychain)
    }

    #[expect(
        clippy::unused_self,
        reason = "same signature as the file backend on other systems"
    )]
    pub fn delete(&self) -> Result<(), KeyStoreError> {
        CALLS
            .write(keychain_wait::DEADLINE, delete_now)
            .map_err(unanswered)?
            .map_err(KeyStoreError::Keychain)
    }
}

/// It can wait for the person for ever, so it runs on the thread of [`CALLS`].
fn read_now() -> Read {
    match entry()?.get_password() {
        Ok(text) => Ok(Some(Zeroizing::new(text.into_bytes()))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(text(&error)),
    }
}

fn write_now(record: &str) -> Written {
    entry()?.set_password(record).map_err(|error| text(&error))
}

fn delete_now() -> Written {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(text(&error)),
    }
}

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT).map_err(|error| text(&error))
}

fn unanswered(error: Unanswered) -> KeyStoreError {
    match error {
        Unanswered::Waits => KeyStoreError::KeychainWaits,
        Unanswered::Lost => KeyStoreError::Keychain(error.to_string()),
    }
}

/// `BadEncoding` carries the item's bytes, which may be a value (LOG-007).
fn text(error: &keyring::Error) -> String {
    match error {
        keyring::Error::BadEncoding(_) => "the item is not text".to_owned(),
        other => other.to_string(),
    }
}
