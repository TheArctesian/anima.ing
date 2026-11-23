//! A secret opens only for the device host it was entered for (SEC-011).
//!
//! A host-bound row uses the context `credentials.secrets.host:<device_id>:<kind>` and the
//! plaintext frame: the host's length as 2 bytes big-endian, the host text, then the secret. A
//! row from before SEC-011 uses `credentials.secrets:<device_id>:<kind>`, holds only the secret
//! and stands for its `legacy_host`. The two contexts never open each other's rows.
//! The host is the configured host text, compared without letter case, never a resolved address.

use zeroize::Zeroizing;

use crate::ids::DeviceId;

use super::{Error, Sealed, Secret, Vault, store::StoredSecret};

pub(super) fn context(device: DeviceId, kind: &str) -> Vec<u8> {
    format!("credentials.secrets.host:{device}:{kind}").into_bytes()
}

pub(super) fn frame(host: &str, secret: &Secret) -> Result<Secret, Error> {
    let length = u16::try_from(host.len()).map_err(|_| Error::Malformed)?;
    let mut plain = Zeroizing::new(Vec::with_capacity(2 + host.len() + secret.expose().len()));
    plain.extend_from_slice(&length.to_be_bytes());
    plain.extend_from_slice(host.as_bytes());
    plain.extend_from_slice(secret.expose());
    Ok(Secret::new(std::mem::take(&mut *plain)))
}

pub(super) fn unframe(plain: &[u8]) -> Result<(&str, Secret), Error> {
    let (length, rest) = plain.split_first_chunk::<2>().ok_or(Error::Malformed)?;
    let (host, secret) = rest
        .split_at_checked(usize::from(u16::from_be_bytes(*length)))
        .ok_or(Error::Malformed)?;
    let host = std::str::from_utf8(host).map_err(|_| Error::Malformed)?;
    Ok((host, Secret::new(secret.to_vec())))
}

pub(super) fn permits(bound: &str, host: &str) -> bool {
    bound.eq_ignore_ascii_case(host)
}

pub(super) fn seal(
    vault: &Vault,
    (device, host): (DeviceId, &str),
    kind: &str,
    secret: &Secret,
) -> Result<Sealed, Error> {
    vault.seal(&frame(host, secret)?, &context(device, kind))
}

/// The opened secret is dropped, its bytes zeroed, before a refusal returns.
pub(super) fn open(
    vault: &Vault,
    row: StoredSecret,
    (device, host): (DeviceId, &str),
    legacy: &[u8],
) -> Result<Secret, Error> {
    let sealed = Sealed::from_parts(&row.nonce, row.ciphertext, row.key_id)?;
    let (secret, permitted) = if let Some(bound) = row.legacy_host {
        let secret = vault.open(&sealed, legacy)?;
        (secret, permits(&bound, host))
    } else {
        let plain = vault.open(&sealed, &context(device, &row.kind))?;
        let (bound, secret) = unframe(plain.expose())?;
        (secret, permits(bound, host))
    };
    if permitted {
        Ok(secret)
    } else {
        Err(Error::EnteredForOtherHost)
    }
}
