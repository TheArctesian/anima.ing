//! Check secrets, sealed to their check and to where they may be sent (SEC-003, SEC-005, D-5).
//! Every open checks the binding against the stored configuration, so a database writer who
//! changes where a check sends its secrets gets an error instead of the secrets.
//! The SNMP host is the configured host text, compared without case, never a resolved address (SEC-010).

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use zeroize::Zeroizing;

use crate::{
    fleet::{
        checks::{
            Error,
            config::{
                ConfigError, HttpAuth, HttpConfig, HttpSecrets, Reason, SnmpConfig, SnmpSecrets,
                SnmpVersion,
            },
            rules::{config_error, origin},
        },
        credentials::{self, Sealed, Secret, Vault},
    },
    ids::CheckId,
};

const HTTP: &str = "checks.checks.http";
const SNMP: &str = "checks.checks.snmp";

const COMMUNITY: u8 = 1;
const USER: u8 = 2;

fn context(prefix: &str, check: CheckId) -> Vec<u8> {
    format!("{prefix}:{check}").into_bytes()
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HttpBinding {
    origin: String,
    headers: Vec<String>,
    auth: HttpAuth,
    verify_tls: bool,
}

impl HttpBinding {
    fn of(config: &HttpConfig) -> Option<Self> {
        Some(Self {
            origin: origin(&config.url)?,
            headers: config
                .headers
                .iter()
                .filter(|header| header.value.is_none())
                .map(|header| header.name.to_ascii_lowercase())
                .collect(),
            auth: config.auth.clone(),
            verify_tls: config.verify_tls,
        })
    }

    /// Everything must match, except that TLS checks may be turned on (SEC-005).
    fn permits(&self, config: &HttpConfig) -> bool {
        Self::of(config).is_some_and(|now| {
            self.origin == now.origin
                && self.headers == now.headers
                && self.auth == now.auth
                && (now.verify_tls || !self.verify_tls)
        })
    }
}

/// `host` is `None` only in a binding sealed before SEC-010.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnmpBinding {
    version: SnmpVersion,
    username: Option<String>,
    #[serde(default)]
    host: Option<String>,
}

impl SnmpBinding {
    fn of(config: &SnmpConfig, host: &str) -> Self {
        Self {
            version: config.version,
            username: config.username.clone(),
            host: Some(host.to_owned()),
        }
    }

    fn permits(
        &self,
        config: &SnmpConfig,
        host: &str,
        legacy_host: Option<&str>,
    ) -> Result<(), Error> {
        if self.version != config.version || self.username != config.username {
            return Err(Error::SecretsUnbound);
        }
        match self.host.as_deref().or(legacy_host) {
            Some(bound) if bound.eq_ignore_ascii_case(host) => Ok(()),
            Some(_) => Err(Error::SecretsForOtherHost),
            None => Err(Error::SecretsUnbound),
        }
    }
}

fn malformed() -> Error {
    Error::Credentials(credentials::Error::Malformed)
}

/// The plaintext to seal: a 4-byte big-endian length, the binding, then the secrets.
fn frame(binding: &[u8], secrets: &Secret) -> Secret {
    let length = u32::try_from(binding.len()).unwrap_or(u32::MAX);
    let mut plain = Zeroizing::new(Vec::with_capacity(
        4 + binding.len() + secrets.expose().len(),
    ));
    plain.extend_from_slice(&length.to_be_bytes());
    plain.extend_from_slice(binding);
    plain.extend_from_slice(secrets.expose());
    Secret::new(std::mem::take(&mut *plain))
}

fn unframe(plain: &[u8]) -> Result<(&[u8], &[u8]), Error> {
    let (length, rest) = plain.split_first_chunk::<4>().ok_or_else(malformed)?;
    let length = usize::try_from(u32::from_be_bytes(*length)).map_err(|_| malformed())?;
    if length > rest.len() {
        return Err(malformed());
    }
    Ok(rest.split_at(length))
}

fn seal_framed(
    vault: &Vault,
    context: &[u8],
    binding: &impl Serialize,
    secrets: &Secret,
) -> Result<Vec<u8>, Error> {
    let binding = serde_json::to_vec(binding)
        .expect("a binding of plain strings, bools and enum tags always serialises (CQ-023)");
    Ok(vault.seal(&frame(&binding, secrets), context)?.to_bytes())
}

/// The binding is not yet checked.
fn open_framed<B: DeserializeOwned>(
    vault: &Vault,
    context: &[u8],
    bytes: &[u8],
) -> Result<(B, Secret), Error> {
    let plain = vault.open(&Sealed::from_bytes(bytes)?, context)?;
    let (binding, secrets) = unframe(plain.expose())?;
    let binding = serde_json::from_slice(binding).map_err(|_| malformed())?;
    Ok((binding, Secret::new(secrets.to_vec())))
}

pub(in crate::fleet::checks) fn seal_http(
    vault: &Vault,
    check: CheckId,
    config: &HttpConfig,
    secrets: &HttpSecrets,
) -> Result<Vec<u8>, Error> {
    let binding = HttpBinding::of(config).ok_or_else(|| {
        Error::Validation(vec![config_error(ConfigError {
            field: "url",
            reason: Reason::Invalid,
        })])
    })?;
    seal_framed(vault, &context(HTTP, check), &binding, &secrets.encode())
}

pub(in crate::fleet::checks) fn open_http(
    vault: &Vault,
    check: CheckId,
    config: &HttpConfig,
    bytes: &[u8],
) -> Result<HttpSecrets, Error> {
    let (binding, secrets) = open_framed::<HttpBinding>(vault, &context(HTTP, check), bytes)?;
    if !binding.permits(config) {
        return Err(Error::SecretsUnbound);
    }
    HttpSecrets::decode(&secrets).map_err(|_| malformed())
}

pub(in crate::fleet::checks) fn seal_snmp(
    vault: &Vault,
    check: CheckId,
    (config, host): (&SnmpConfig, &str),
    secrets: &SnmpSecrets,
) -> Result<Vec<u8>, Error> {
    let binding = SnmpBinding::of(config, host);
    seal_framed(vault, &context(SNMP, check), &binding, &encode(secrets))
}

/// `legacy_host` is the stored `snmp_legacy_host`, used only when the binding names no host.
pub(in crate::fleet::checks) fn open_snmp(
    vault: &Vault,
    check: CheckId,
    (config, host): (&SnmpConfig, &str),
    (bytes, legacy_host): (&[u8], Option<&str>),
) -> Result<SnmpSecrets, Error> {
    let (binding, secrets) = open_framed::<SnmpBinding>(vault, &context(SNMP, check), bytes)?;
    binding.permits(config, host, legacy_host)?;
    let secrets = decode(secrets.expose())?;
    if secrets.version() == config.version {
        Ok(secrets)
    } else {
        Err(malformed())
    }
}

/// The SNMP secrets in the sealed layout, in a buffer sized up front so it never grows.
fn encode(secrets: &SnmpSecrets) -> Secret {
    let (kind, parts): (u8, Vec<&Secret>) = match secrets {
        SnmpSecrets::Community(community) => (COMMUNITY, vec![community]),
        SnmpSecrets::User { auth, privacy } => (USER, vec![auth, privacy]),
    };
    let size = 1 + parts.iter().map(|p| 2 + p.expose().len()).sum::<usize>();
    let mut bytes = Zeroizing::new(Vec::with_capacity(size));
    bytes.push(kind);
    for part in parts {
        let length = u16::try_from(part.expose().len()).unwrap_or(u16::MAX);
        bytes.extend_from_slice(&length.to_be_bytes());
        bytes.extend_from_slice(&part.expose()[..usize::from(length)]);
    }
    Secret::new(std::mem::take(&mut *bytes))
}

fn part(bytes: &mut &[u8]) -> Result<Secret, Error> {
    let (length, rest) = bytes.split_first_chunk::<2>().ok_or_else(malformed)?;
    let length = usize::from(u16::from_be_bytes(*length));
    if length > rest.len() {
        return Err(malformed());
    }
    let (value, rest) = rest.split_at(length);
    *bytes = rest;
    Ok(Secret::new(value.to_vec()))
}

fn decode(bytes: &[u8]) -> Result<SnmpSecrets, Error> {
    let (&kind, mut rest) = bytes.split_first().ok_or_else(malformed)?;
    let secrets = match kind {
        COMMUNITY => SnmpSecrets::Community(part(&mut rest)?),
        USER => SnmpSecrets::User {
            auth: part(&mut rest)?,
            privacy: part(&mut rest)?,
        },
        _ => return Err(malformed()),
    };
    if rest.is_empty() {
        Ok(secrets)
    } else {
        Err(malformed())
    }
}
