//! Nothing reads these updater key entries since the hub signs (R-117, D-41); deleting them ends the sealed key.

use keyring::Entry;

use super::{error::KeystoreError, token::SERVICE};

pub(super) const RETIRED_ACCOUNTS: [&str; 2] = ["updater-key", "updater-key-public"];

/// Tries both before it returns the first failure, which carries no backend text (LOG-007).
pub(super) fn forget_updater_key() -> Result<(), KeystoreError> {
    let mut first = Ok(());
    for account in RETIRED_ACCOUNTS {
        let deleted = Entry::new(SERVICE, account).and_then(|entry| entry.delete_credential());
        match deleted {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => first = first.and(Err(error.into())),
        }
    }
    first
}
