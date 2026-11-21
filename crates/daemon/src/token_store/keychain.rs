//! The macOS token store: one generic password in the login Keychain (MET-013).
//!
//! Compiled only on macOS. Each read, write and delete is bounded ([`crate::keychain_wait`]):
//! a call that macOS holds for the person is [`TokenStoreError::KeychainWaits`].

use std::time::Duration;

use keyring::Entry;
use zeroize::Zeroizing;

use super::TokenStoreError;
use crate::config::home::Home;
use crate::keychain_wait::{Calls, Read, Unanswered, Written};

/// The calls of the token's item: one in flight at a time (R-322).
static CALLS: Calls = Calls::new();

const SERVICE: &str = "ing.anima.daemon";
const ACCOUNT: &str = "daemon-token";

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
    pub fn read(&self, deadline: Duration) -> Result<Option<Zeroizing<Vec<u8>>>, TokenStoreError> {
        CALLS
            .read(deadline, read_now)
            .map_err(unanswered)?
            .map_err(TokenStoreError::Keychain)
    }

    /// Writes the record. A `fresh` write (enrolment) deletes the item first, so the new item
    /// gets a fresh access list rather than inheriting one a stale program was added to.
    #[expect(
        clippy::unused_self,
        reason = "same signature as the file backend on other systems"
    )]
    pub fn write(
        &self,
        bytes: &[u8],
        fresh: bool,
        deadline: Duration,
    ) -> Result<(), TokenStoreError> {
        let text = std::str::from_utf8(bytes).map_err(|_| TokenStoreError::Corrupt)?;
        let text = Zeroizing::new(text.to_owned());
        CALLS
            .write(deadline, move || write_now(&text, fresh))
            .map_err(unanswered)?
            .map_err(TokenStoreError::Keychain)
    }

    /// Deletes the item; an item that is not there is not an error.
    #[expect(
        clippy::unused_self,
        reason = "same signature as the file backend on other systems"
    )]
    pub fn delete(&self, deadline: Duration) -> Result<(), TokenStoreError> {
        CALLS
            .write(deadline, delete_now)
            .map_err(unanswered)?
            .map_err(TokenStoreError::Keychain)
    }
}

/// One read of the item, on the thread of [`CALLS`]: it can wait for the person for ever.
fn read_now() -> Read {
    match entry()?.get_password() {
        Ok(text) => Ok(Some(Zeroizing::new(text.into_bytes()))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

/// One write of the item, on the thread of [`CALLS`]; `fresh` deletes it first.
fn write_now(text: &str, fresh: bool) -> Written {
    if fresh {
        delete_now()?;
    }
    entry()?
        .set_password(text)
        .map_err(|error| error.to_string())
}

/// One delete of the item, on the thread of [`CALLS`].
fn delete_now() -> Written {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

fn entry() -> Result<Entry, String> {
    Entry::new(SERVICE, ACCOUNT).map_err(|error| error.to_string())
}

fn unanswered(error: Unanswered) -> TokenStoreError {
    match error {
        Unanswered::Waits => TokenStoreError::KeychainWaits,
        Unanswered::Lost => TokenStoreError::Keychain(error.to_string()),
    }
}
