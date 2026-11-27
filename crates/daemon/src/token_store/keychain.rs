//! The macOS token store: one generic password in the login Keychain (MET-013).
//! Each read, write and delete is bounded ([`crate::keychain_wait`]): a call that macOS holds for
//! the person is [`TokenStoreError::KeychainWaits`].

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
pub struct Backend(Home);

impl Backend {
    pub fn open(home: &Home) -> Self {
        Self(home.clone())
    }

    pub fn read(&self, deadline: Duration) -> Result<Option<Zeroizing<Vec<u8>>>, TokenStoreError> {
        let bytes = CALLS
            .read(deadline, read_now)
            .map_err(unanswered)?
            .map_err(TokenStoreError::Keychain)?;
        if let Some(bytes) = bytes.as_ref()
            && !super::holder_authority::owns(bytes, &self.folder()?)
        {
            return Err(TokenStoreError::Corrupt);
        }
        Ok(bytes)
    }

    pub(super) fn folder(&self) -> Result<super::holder_authority::Folder, TokenStoreError> {
        super::holder_authority::Folder::at(self.0.dir()).ok_or(TokenStoreError::Corrupt)
    }

    /// A `fresh` write deletes the item first, so it does not inherit an access list a stale program was added to.
    pub fn write(
        &self,
        bytes: &[u8],
        device: Option<uuid::Uuid>,
        deadline: Duration,
    ) -> Result<(), TokenStoreError> {
        let text = std::str::from_utf8(bytes).map_err(|_| TokenStoreError::Corrupt)?;
        let text = Zeroizing::new(text.to_owned());
        let folder = self.folder()?;
        CALLS
            .write(deadline, move || write_now(&text, device, folder))
            .map_err(unanswered)?
            .map_err(TokenStoreError::Keychain)
    }

    pub fn delete(&self, deadline: Duration) -> Result<(), TokenStoreError> {
        CALLS
            .write(deadline, {
                let home = self.folder()?;
                move || {
                    if let Some(bytes) = read_now()?
                        && !super::holder_authority::owns(&bytes, &home)
                    {
                        return Err("credential belongs to another home".to_owned());
                    }
                    delete_now()
                }
            })
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

fn write_now(
    text: &str,
    device: Option<uuid::Uuid>,
    home: super::holder_authority::Folder,
) -> Written {
    let fresh = device.is_some();
    let previous = if fresh { None } else { read_now()? };
    let text = super::holder_authority::record(
        text,
        home,
        device,
        previous.as_deref().map(Vec::as_slice),
    )?;
    if fresh {
        delete_now()?;
    }
    entry()?
        .set_password(&text)
        .map_err(|error| error.to_string())
}

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
