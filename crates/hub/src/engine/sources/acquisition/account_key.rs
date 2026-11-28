//! A key of the account and the base of the operator's server (K-E1 Section 7b, R-261).
//! `held` opens the stored key for one request and keeps it in no cache.

use reqwest::Url;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{
    engine::sources::{
        Scheduling,
        model::{Account, ErrorKind},
        run::Failure,
    },
    fleet::credentials::{self, Secret, load_source_secret},
};

const VERSION: u8 = 1;
const MAX_KEY: usize = 512;
const MAX_BASE: usize = 300;

/// Fixed words: never the address or the key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    #[error("the address is not https://host[:port] or http://host[:port]")]
    Base,
    #[error("the key is not 1 to 512 characters with no control character")]
    Key,
}

/// `https` or `http`; the transport sends plain `http` only to a private or VPN address
/// (OD-E26).
///
/// # Errors
/// [`Refusal::Base`].
pub fn base(text: &str) -> Result<Url, Refusal> {
    let text = text.trim();
    let url = Url::parse(text)
        .ok()
        .filter(|_| text.chars().count() <= MAX_BASE)
        .ok_or(Refusal::Base)?;
    let scheme = matches!(url.scheme(), "https" | "http");
    let bare = url.username().is_empty()
        && url.password().is_none()
        && matches!(url.path(), "" | "/")
        && url.query().is_none()
        && url.fragment().is_none();
    if !scheme || !bare || url.host_str().is_none() {
        return Err(Refusal::Base);
    }
    Url::parse(&url.origin().ascii_serialization()).map_err(|_| Refusal::Base)
}

#[must_use]
pub fn instance_key(base: &Url) -> String {
    base.origin().ascii_serialization()
}

/// # Errors
/// [`Refusal::Key`].
pub fn key(text: &str) -> Result<&str, Refusal> {
    let text = text.trim();
    let valid = !text.is_empty()
        && text.chars().count() <= MAX_KEY
        && !text.chars().any(|c| c.is_control() || c.is_whitespace());
    if valid { Ok(text) } else { Err(Refusal::Key) }
}

#[derive(Serialize)]
struct Stored<'a> {
    v: u8,
    base: &'a str,
    api_key: &'a str,
}

#[derive(Deserialize)]
struct Opened {
    v: u8,
    base: String,
    api_key: String,
}

/// The plaintext to seal: `{"v":1,"base","api_key"}`.
///
/// # Panics
/// Never: strings and a number always serialise.
#[must_use]
pub fn to_secret(base: &Url, api_key: &str) -> Secret {
    let origin = instance_key(base);
    let stored = Stored {
        v: VERSION,
        base: &origin,
        api_key,
    };
    let worst = 6 * (origin.len() + api_key.len()) + 64;
    let mut plain = Zeroizing::new(Vec::with_capacity(worst));
    serde_json::to_writer(&mut *plain, &stored)
        .expect("strings and a number always serialise (CQ-023)");
    Secret::new(std::mem::take(&mut *plain))
}

/// The base must be the `instance_key`, so a database writer cannot point the key elsewhere.
#[must_use]
pub fn from_secret(secret: &Secret, instance_key: &str) -> Option<(Url, Secret)> {
    let opened: Opened = serde_json::from_slice(secret.expose()).ok()?;
    let api_key = Zeroizing::new(opened.api_key);
    if opened.v != VERSION || opened.base != instance_key {
        return None;
    }
    let url = base(&opened.base).ok()?;
    Some((url, Secret::new(api_key.as_bytes().to_vec())))
}

pub(in crate::engine::sources) async fn held(
    scheduling: &Scheduling,
    account: &Account,
) -> Result<(Url, Secret), Failure> {
    let reconnect = |words| Failure::new(ErrorKind::Auth, words);
    let credential = account
        .credential_id
        .ok_or(reconnect("No key is stored. Connect the account again."))?;
    let stored = (credential, account.id);
    let secret = load_source_secret(&scheduling.pool, &scheduling.vault, stored)
        .await
        .map_err(|error| match error {
            credentials::Error::Database(_) => Failure::store(),
            _ => reconnect("The stored key cannot be opened. Connect the account again."),
        })?;
    from_secret(&secret, &account.instance_key).ok_or(reconnect(
        "The stored key does not belong to this account. Connect the account again.",
    ))
}
