//! The signed list of runtimes (R-347, R-NEW-launcherlist; contract K11 Section 9.1): the SHA-256
//! of each runtime program that the launcher may run. The release workflow signs it with the
//! updater key; the trusted comment names the file and the sequence of the list.

use serde::Deserialize;
use updater_signature::{PublicKey, SignatureCheck};

/// The name of the list file, which the trusted comment of its signature must name.
pub const FILE: &str = "runtime-list.json";
/// The name of the signature file of the list.
pub const SIGNATURE_FILE: &str = "runtime-list.json.sig";
/// The largest list that is read; a real one is a few kilobytes.
pub const MAX_BYTES: usize = 1 << 20;

/// One runtime program that the launcher may run.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub runtime: String,
    pub version: String,
    pub backend: String,
    pub os: String,
    pub architecture: String,
    /// The path of the program below the folder of the release.
    pub program: String,
    /// The SHA-256 of the program file after the unpack, in lower-case hex.
    pub sha256: String,
}

/// A list whose signature holds.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct RuntimeList {
    /// Each change of the list makes it larger.
    pub sequence: u64,
    pub runtimes: Vec<Entry>,
}

/// Why a list is not taken. One answer for each: the caller refuses the start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Over [`MAX_BYTES`], or not a list.
    Malformed,
    /// The signature does not hold for these bytes, this file name and this sequence.
    Signature,
}

/// Reads `bytes`, a list as it came, and checks `signature`, the text of its `.sig` file, with
/// `key`.
///
/// # Errors
/// [`Refusal::Malformed`] when the bytes are too long or are not a list;
/// [`Refusal::Signature`] when the signature does not hold for them, for [`FILE`] and for the
/// sequence that they hold.
pub fn read_signed(bytes: &[u8], signature: &str, key: &PublicKey) -> Result<RuntimeList, Refusal> {
    if bytes.len() > MAX_BYTES {
        return Err(Refusal::Malformed);
    }
    // The sequence is read before the signature is checked, because the signature names it; no
    // other value of the list is used before the check holds.
    let list: RuntimeList = serde_json::from_slice(bytes).map_err(|_| Refusal::Malformed)?;
    let check = SignatureCheck::new(key, signature).map_err(|_| Refusal::Signature)?;
    let mut stream = check.stream().map_err(|_| Refusal::Signature)?;
    stream.update(bytes);
    stream
        .finish_named(&list.sequence.to_string(), Some(FILE))
        .map_err(|_| Refusal::Signature)?;
    Ok(list)
}

impl RuntimeList {
    /// Whether the list has a program with this SHA-256 (lower-case hex) for this operating
    /// system and architecture.
    #[must_use]
    pub fn allows(&self, sha256: &str, os: &str, architecture: &str) -> bool {
        self.runtimes.iter().any(|entry| {
            entry.sha256.len() == 64
                && entry.sha256.eq_ignore_ascii_case(sha256)
                && entry.os == os
                && entry.architecture == architecture
        })
    }
}

/// The SHA-256 of `bytes` in lower-case hex.
#[must_use]
pub fn hex(digest: &[u8]) -> String {
    use std::fmt::Write as _;
    digest.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}
