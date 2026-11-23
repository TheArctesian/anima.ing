//! The sealed form of one version of an operational key (SEC-003, R-147).
//! Sealed under its version's context, so a value copied to another version row does not open (K-E5 Section 2).

use crate::{
    fleet::credentials::{Error as CredentialError, Sealed, Secret, Vault},
    ids::OperationalKeyVersionId,
};

const CONTEXT: &str = "credentials.operational_key_versions";

fn context(version: OperationalKeyVersionId) -> Vec<u8> {
    format!("{CONTEXT}:{version}").into_bytes()
}

pub(super) fn seal(
    vault: &Vault,
    version: OperationalKeyVersionId,
    value: &Secret,
) -> Result<Vec<u8>, CredentialError> {
    Ok(vault.seal(value, &context(version))?.to_bytes())
}

/// A value deleted after its version ended is empty, so it opens as `Malformed`.
pub(super) fn open(
    vault: &Vault,
    version: OperationalKeyVersionId,
    stored: &[u8],
) -> Result<Secret, CredentialError> {
    vault.open(&Sealed::from_bytes(stored)?, &context(version))
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;
    use zeroize::Zeroizing;

    use super::*;

    fn vault() -> Vault {
        Vault::from_key(Zeroizing::new([9_u8; 32]))
    }

    fn version(n: u128) -> OperationalKeyVersionId {
        Uuid::from_u128(n).into()
    }

    #[test]
    fn sec_003_a_value_opens_only_for_its_own_version() {
        let vault = vault();
        let sealed = seal(&vault, version(1), &Secret::from("api-123".to_owned())).unwrap();
        let opened = open(&vault, version(1), &sealed).unwrap();
        assert_eq!(opened.expose(), b"api-123");
        assert!(matches!(
            open(&vault, version(2), &sealed),
            Err(CredentialError::Decrypt)
        ));
    }

    #[test]
    fn r_147_a_forgotten_value_does_not_open() {
        assert!(matches!(
            open(&vault(), version(1), &[]),
            Err(CredentialError::Malformed)
        ));
    }
}
