//! The account that a program uses, as it may leave the machine (AIS-022 as THR-149 changed
//! it): a key made from the vendor's identifier with the salt of the hub, the email address of
//! the account, the name of the plan, and the limit windows. Never the identifier or a token.
//! The address is personal data: its `Debug` shows no text, so no log line holds it (LOG-007).
//! Pure.

use std::fmt::{self, Write as _};

use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit as _, Mac as _};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::Sha256;

use super::Source;

/// The most accounts one batch may hold: one for each source, with room for newer sources.
pub const MAX_ACCOUNTS: usize = 4;
/// The most limit windows of one account.
pub const MAX_LIMITS: usize = 8;
/// The bytes of the account salt of a hub.
pub const SALT_BYTES: usize = 32;
/// The hexadecimal digits of a key.
pub const KEY_DIGITS: usize = 64;
/// The most bytes of an address (RFC 5321 Section 4.5.3.1.3).
pub const MAX_ADDRESS: usize = 254;
const MAX_PLAN: usize = 32;
const MAX_WINDOW: usize = 32;

/// One account of one source on the machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub source: Source,
    /// [`account_key`] of the vendor's identifier.
    pub key: String,
    /// The email address that the program is signed in with, as the program gives it.
    pub address: Option<Address>,
    pub plan: Option<String>,
    pub limits: Vec<Limit>,
}

/// The email address of an account (THR-149): personal data. It is shown to the person's
/// clients, and its `Debug` shows no text, so that a log line or an error made from an account
/// cannot hold it (LOG-007, THR-149 point 7).
#[derive(Clone, PartialEq, Eq)]
pub struct Address(String);

impl Address {
    /// `text` as an address when [`is_address`] holds.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        is_address(text).then(|| Self(text.to_owned()))
    }

    /// The text, for the store of the hub and for the person's clients only.
    #[must_use]
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl Serialize for Address {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// A text that is not an address is an error that does not repeat the text.
impl<'de> Deserialize<'de> for Address {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).ok_or_else(|| serde::de::Error::custom("not an email address"))
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Address(..)")
    }
}

/// One limit window of an account: how much of it is used, and when it starts again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Limit {
    /// The name of the window as the program gives it (`five_hour`), or its length (`300 min`).
    pub window: String,
    pub used_percent: Option<u8>,
    pub resets_at: Option<DateTime<Utc>>,
    pub reached: bool,
}

/// The key of the account `identifier` of `source` with the account salt of a hub: the
/// HMAC-SHA-256 of `<source>:<identifier>`, in lower-case hexadecimal (K8 Section 4). The same
/// account on two machines of one hub gives one key; a different salt gives a different key, and
/// the key does not give the identifier back.
///
/// # Panics
/// Never: HMAC takes a key of any length.
#[must_use]
pub fn account_key(salt: &[u8; SALT_BYTES], source: Source, identifier: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(salt).expect("HMAC takes a key of any length");
    mac.update(source.as_str().as_bytes());
    mac.update(b":");
    mac.update(identifier.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .fold(String::with_capacity(KEY_DIGITS), |mut key, byte| {
            let _ = write!(key, "{byte:02x}");
            key
        })
}

/// Whether `text` is a key: [`KEY_DIGITS`] lower-case hexadecimal digits.
#[must_use]
pub fn is_key(text: &str) -> bool {
    text.len() == KEY_DIGITS
        && text
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

/// Whether `text` is an email address as a program gives it: 3 to [`MAX_ADDRESS`] bytes, one
/// `@` with text on each side, and no space, control character or angle bracket.
#[must_use]
pub fn is_address(text: &str) -> bool {
    let Some((local, domain)) = text.split_once('@') else {
        return false;
    };
    (3..=MAX_ADDRESS).contains(&text.len())
        && !local.is_empty()
        && !domain.is_empty()
        && !domain.contains('@')
        && !text
            .chars()
            .any(|each| each.is_whitespace() || each.is_control() || matches!(each, '<' | '>'))
}

/// Whether `text` is the name of a plan: 1 to 32 bytes of `[A-Za-z0-9 ._-]`.
#[must_use]
pub fn is_plan(text: &str) -> bool {
    (1..=MAX_PLAN).contains(&text.len())
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b' ' | b'.' | b'_' | b'-'))
}

/// Whether `text` is the name of a limit window: 1 to 32 bytes of `[a-z0-9_ ]`.
#[must_use]
pub fn is_window(text: &str) -> bool {
    (1..=MAX_WINDOW).contains(&text.len())
        && text.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b' ')
        })
}
