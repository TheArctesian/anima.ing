//! The sealed part of a minisign secret key; its scrypt cost is capped (BLD-045).

use blake2::{Blake2b256, Digest};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use crate::{
    error::SignatureError,
    key_file::{CHK_BYTES, MEM_AT, OPS_AT, SALT_AT, SEALED_AT, SEALED_BYTES, SIG_ALG},
};

/// minisign's ceiling: scrypt `N` at most 2^20 (1 GiB with r = 8); real key files use p = 1.
const N_LOG2_MAX: u8 = 20;
const P_MAX: u32 = 1;

pub(crate) fn checksum_holds(bytes: &[u8]) -> bool {
    let sealed = &bytes[SEALED_AT..];
    let (body, stored) = sealed.split_at(SEALED_BYTES - CHK_BYTES);
    checksum(body).as_slice().ct_eq(stored).into()
}

fn checksum(body: &[u8]) -> [u8; CHK_BYTES] {
    let mut hash = Blake2b256::new();
    hash.update(SIG_ALG);
    hash.update(body);
    hash.finalize().into()
}

fn cost_of(bytes: &[u8]) -> Option<(u8, u32)> {
    let ops = u64::from_le_bytes(bytes[OPS_AT..MEM_AT].try_into().ok()?);
    let mem = u64::from_le_bytes(bytes[MEM_AT..SEALED_AT].try_into().ok()?);
    scrypt_cost(ops, mem)
}

pub(crate) fn unseal(bytes: &mut [u8], password: &[u8]) -> Result<(), SignatureError> {
    let (n_log2, p) = cost_of(bytes).ok_or(SignatureError::Format)?;
    let params = scrypt::Params::new(n_log2, 8, p).map_err(|_| SignatureError::Format)?;
    let mut stream = Zeroizing::new([0; SEALED_BYTES]);
    scrypt::scrypt(
        password,
        &bytes[SALT_AT..OPS_AT],
        &params,
        stream.as_mut_slice(),
    )
    .map_err(|_| SignatureError::Format)?;
    for (byte, mask) in bytes[SEALED_AT..].iter_mut().zip(stream.iter()) {
        *byte ^= mask;
    }
    Ok(())
}

/// The scrypt cost as libsodium's `pickparams` computes it; `None` above the ceilings.
pub(crate) fn scrypt_cost(opslimit: u64, memlimit: u64) -> Option<(u8, u32)> {
    const R: u64 = 8;
    let ops = opslimit.max(32_768);
    let smallest_above = |max_n: u64| {
        let mut n_log2 = 1_u8;
        while n_log2 < 63 && (1_u64 << n_log2) <= max_n / 2 {
            n_log2 += 1;
        }
        n_log2
    };
    let (n_log2, p) = if ops < memlimit / 32 {
        (smallest_above(ops / (R * 4)), 1)
    } else {
        let n_log2 = smallest_above(memlimit / (R * 128));
        let rp = ((ops / 4) >> n_log2).min(0x3fff_ffff);
        (n_log2, u32::try_from(rp / R).ok()?)
    };
    (n_log2 <= N_LOG2_MAX && (1..=P_MAX).contains(&p)).then_some((n_log2, p))
}
