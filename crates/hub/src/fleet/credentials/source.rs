//! A source secret is sealed under `credentials.source_secrets:<account_id>` (hyphenated
//! lowercase UUID), so a row moved or copied to another account does not open (SRC-008, PRJ-018, SEC-003, K3).

use crate::ids::SourceAccountId;

use super::{Error, Sealed, Secret, Vault, store::StoredSourceSecret};

const CONTEXT: &str = "credentials.source_secrets";

pub(super) fn context(account: SourceAccountId) -> Vec<u8> {
    format!("{CONTEXT}:{account}").into_bytes()
}

pub(super) fn seal(
    vault: &Vault,
    account: SourceAccountId,
    secret: &Secret,
) -> Result<Sealed, Error> {
    vault.seal(secret, &context(account))
}

pub(super) fn open(
    vault: &Vault,
    row: StoredSourceSecret,
    account: SourceAccountId,
) -> Result<Secret, Error> {
    let sealed = Sealed::from_parts(&row.nonce, row.ciphertext, row.key_id)?;
    vault.open(&sealed, &context(account))
}
