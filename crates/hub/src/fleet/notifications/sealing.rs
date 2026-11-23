//! Channel secrets sealed per channel and bound to kind, target and header names (SEC-003, D-17, K3).
//! A channel pointed at another target opens to an error, never the secrets, and nothing is sent (D-5).

use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::{
    Error,
    channels::{Kept, Secrets, Settings, Wire},
};
use crate::{
    fleet::credentials::{self, Sealed, Secret, Vault},
    ids::ChannelId,
};

const CONTEXT: &str = "notifications.channels";

fn context(channel: ChannelId) -> Vec<u8> {
    format!("{CONTEXT}:{channel}").into_bytes()
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<'a> {
    #[serde(borrow)]
    kind: Cow<'a, str>,
    #[serde(borrow)]
    target: Cow<'a, str>,
    #[serde(borrow)]
    names: Vec<Cow<'a, str>>,
    #[serde(borrow)]
    secrets: Wire<'a>,
}

pub(super) fn seal(
    vault: &Vault,
    channel: ChannelId,
    settings: &Settings,
    secrets: &Secrets,
) -> Result<Vec<u8>, Error> {
    let target = settings.target();
    let names = settings.secret_names();
    let envelope = Envelope {
        kind: Cow::Borrowed(settings.kind().as_str()),
        target: Cow::Borrowed(&target),
        names: names
            .iter()
            .map(|name| Cow::Borrowed(name.as_str()))
            .collect(),
        secrets: secrets.to_wire(),
    };
    // Sized for worst-case escaping so the buffer never grows and leaves no copy of a secret.
    let clear: usize = [target.len(), names.iter().map(String::len).sum()]
        .iter()
        .sum();
    let worst = envelope.secrets.worst_len() + 6 * clear + 8 * names.len() + 128;
    let mut plain = Zeroizing::new(Vec::with_capacity(worst));
    serde_json::to_writer(&mut *plain, &envelope)
        .expect("strings and lists of strings always serialise (CQ-023)");
    let plain = Secret::new(std::mem::take(&mut *plain));
    Ok(vault.seal(&plain, &context(channel))?.to_bytes())
}

pub(super) fn open(
    vault: &Vault,
    channel: ChannelId,
    settings: &Settings,
    bytes: &[u8],
) -> Result<Secrets, Error> {
    let plain = vault.open(&Sealed::from_bytes(bytes)?, &context(channel))?;
    let envelope: Envelope<'_> = serde_json::from_slice(plain.expose())
        .map_err(|_| Error::Credentials(credentials::Error::Malformed))?;
    let names = settings.secret_names();
    let bound = envelope.kind == settings.kind().as_str()
        && envelope.target == settings.target()
        && envelope
            .names
            .iter()
            .map(Cow::as_ref)
            .eq(names.iter().map(String::as_str));
    if !bound {
        return Err(Error::SecretsUnbound);
    }
    Secrets::from_wire(settings, envelope.secrets).ok_or(Error::SecretsUnbound)
}

pub(super) fn kept(
    vault: &Vault,
    channel: ChannelId,
    settings: &Settings,
    bytes: Option<&[u8]>,
) -> Kept<Secrets> {
    match bytes.map(|bytes| open(vault, channel, settings, bytes)) {
        None => Kept::Nothing,
        Some(Ok(secrets)) => Kept::Opened(secrets),
        Some(Err(_)) => Kept::Unopened,
    }
}
