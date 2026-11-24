//! `num-bigint` numbers cannot be zeroed, so only the byte copies of the secrets are zeroed.

use aes::cipher::{BlockCipherEncrypt as _, KeyInit as _};
use md5::{Digest as _, Md5};
use num_bigint::BigUint;
use zeroize::Zeroizing;

/// The longest account name or password Apple's method carries, in bytes.
pub(super) const APPLE_LONGEST: usize = 63;
/// 1024 bits (what macOS sends) to 4096: a shorter number exposes the password, a longer one costs seconds.
pub(super) const KEY_BYTES: std::ops::RangeInclusive<usize> = 128..=512;

/// VNC authentication (RFC 6143 Section 7.2.2): the challenge encrypted with DES under the
/// password. The password is cut to 8 bytes and each key byte has its bits in reverse
/// order, as every VNC server expects.
pub(super) fn vnc_password_answer(challenge: [u8; 16], password: &str) -> [u8; 16] {
    let mut key = Zeroizing::new([0u8; 8]);
    for (slot, byte) in key.iter_mut().zip(password.bytes()) {
        *slot = byte.reverse_bits();
    }
    let cipher = des::Des::new(&(*key).into());
    let mut answer = challenge;
    let (blocks, _) = answer.as_chunks_mut::<8>();
    for block in blocks {
        cipher.encrypt_block(block.into());
    }
    answer
}

/// What a device sends to open Apple's method (security type 30).
pub(super) struct AppleChallenge {
    pub(super) generator: u16,
    pub(super) prime: Vec<u8>,
    pub(super) server_public: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum NoAppleAnswer {
    TooLong,
    BadNumbers,
}

/// Name and password in zero-ended 64-byte fields, AES-128-ECB under the MD5 of the shared
/// secret, then the client's public number. `private` and `filler` must be random.
pub(super) fn apple_answer(
    challenge: &AppleChallenge,
    (username, password): (&str, &str),
    private: &[u8],
    filler: [u8; 128],
) -> Result<Vec<u8>, NoAppleAnswer> {
    if username.len() > APPLE_LONGEST || password.len() > APPLE_LONGEST {
        return Err(NoAppleAnswer::TooLong);
    }
    let length = challenge.prime.len();
    let prime = BigUint::from_bytes_be(&challenge.prime);
    let server_public = BigUint::from_bytes_be(&challenge.server_public);
    let one = BigUint::from(1u8);
    // The prime must really have its stated length and be odd; a public number of 0, 1 or
    // prime - 1 would make the secret a number anyone knows.
    let usable = KEY_BYTES.contains(&length)
        && prime.bits() > u64::try_from((length - 1) * 8).unwrap_or(u64::MAX)
        && prime.bit(0)
        && challenge.generator > 1
        && server_public > one
        && server_public < &prime - &one;
    if !usable {
        return Err(NoAppleAnswer::BadNumbers);
    }
    let private = BigUint::from_bytes_be(private);
    let public = BigUint::from(challenge.generator).modpow(&private, &prime);
    let secret = Zeroizing::new(padded(&server_public.modpow(&private, &prime), length));
    let key: [u8; 16] = Md5::digest(&*secret).into();
    let key = Zeroizing::new(key);

    let mut fields = Zeroizing::new(filler);
    put(&mut fields[..64], username.as_bytes());
    put(&mut fields[64..], password.as_bytes());
    let cipher = aes::Aes128::new(&(*key).into());
    let (blocks, _) = fields.as_chunks_mut::<16>();
    for block in blocks {
        cipher.encrypt_block(block.into());
    }
    let mut answer = fields.to_vec();
    answer.extend_from_slice(&padded(&public, length));
    Ok(answer)
}

fn put(field: &mut [u8], text: &[u8]) {
    field[..text.len()].copy_from_slice(text);
    field[text.len()] = 0;
}

fn padded(number: &BigUint, length: usize) -> Vec<u8> {
    let bytes = Zeroizing::new(number.to_bytes_be());
    let mut out = vec![0; length.saturating_sub(bytes.len())];
    out.extend_from_slice(&bytes);
    out
}
