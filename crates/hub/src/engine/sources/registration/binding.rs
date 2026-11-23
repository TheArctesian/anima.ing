//! Values that tie a browser to the attempt (K-E1 Section 7a, PRJ-041).
//! The hub keeps only their SHA-256 hashes; no value has `Debug` and the text wipes itself (LOG-007).

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ring::{hmac, rand::SecureRandom};
use sha2::{Digest as _, Sha256};
use subtle::ConstantTimeEq as _;
use zeroize::Zeroizing;

const BYTES: usize = 32;
/// 32 bytes are 43 base64url characters; longer text is never hashed.
const MAX_PRESENTED: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the random source failed")]
pub struct RandomFailed;

#[derive(Clone, Copy)]
pub struct Hash([u8; 32]);

impl Hash {
    fn of(text: &str) -> Self {
        Self(Sha256::digest(text.as_bytes()).into())
    }

    /// Compared in constant time; text longer than a value can be is never a match.
    #[must_use]
    pub fn matches(&self, presented: &str) -> bool {
        presented.len() <= MAX_PRESENTED && bool::from(Self::of(presented).0.ct_eq(&self.0))
    }
}

pub struct Minted {
    text: Zeroizing<String>,
    hash: Hash,
}

impl Minted {
    /// # Errors
    /// [`RandomFailed`] when the random source fails.
    pub fn new(random: &dyn SecureRandom) -> Result<Self, RandomFailed> {
        let mut bytes = Zeroizing::new([0_u8; BYTES]);
        random.fill(&mut *bytes).map_err(|_| RandomFailed)?;
        let text = Zeroizing::new(URL_SAFE_NO_PAD.encode(*bytes));
        let hash = Hash::of(&text);
        Ok(Self { text, hash })
    }

    /// For the one URL, cookie or page it travels in, never for a log.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The same state for a browser and a step on every ask, so a replay replaces nothing; it does
    /// not reveal the cookie.
    #[must_use]
    pub fn state(cookie: &str, step: Step) -> Self {
        let label: &[u8] = match step {
            Step::Create => b"anima connect github: state S1",
            Step::Install => b"anima connect github: state S2",
        };
        let tag = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, cookie.as_bytes()), label);
        let text = Zeroizing::new(URL_SAFE_NO_PAD.encode(tag.as_ref()));
        let hash = Hash::of(&text);
        Self { text, hash }
    }

    #[must_use]
    pub const fn hash(&self) -> Hash {
        self.hash
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Create,
    Install,
}

/// # Errors
/// [`RandomFailed`] when the random source fails.
pub fn name_entropy(random: &dyn SecureRandom) -> Result<[u8; 7], RandomFailed> {
    let mut bytes = [0_u8; 7];
    random.fill(&mut bytes).map_err(|_| RandomFailed)?;
    Ok(bytes)
}
