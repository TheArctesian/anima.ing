//! Validation of operator input for checks; errors name the field, never its value (LOG-007).

use super::{
    config::{
        CheckConfig, ConfigError, HttpAuth, HttpConfig, HttpCredentials, HttpInput, HttpSecrets,
        SnmpConfig, SnmpCredentials, SnmpInput, SnmpSecrets, SnmpVersion, is_secret_header,
    },
    model::{Definition, FieldError, PushToken},
};
use crate::{fleet::credentials::Secret, text::is_plain};

pub(super) enum Sealable {
    Http(HttpSecrets),
    Snmp(SnmpSecrets),
}

pub(super) struct Prepared {
    pub(super) config: CheckConfig,
    pub(super) secrets: Option<Sealable>,
    pub(super) push_token: Option<PushToken>,
}

pub(super) type StoredHttp = (HttpConfig, HttpSecrets);

pub(super) type StoredSnmp = (SnmpConfig, SnmpSecrets);

pub(super) enum Stored {
    Nothing,
    Http(StoredHttp),
    Snmp(StoredSnmp),
}

/// The length limit is the database's (`checks_name_check`).
pub(super) fn name(text: &str) -> Result<String, FieldError> {
    let name = text.trim();
    if name.is_empty() {
        return Err(FieldError::new("name", "Enter a name for the check."));
    }
    if !is_plain(name) {
        return Err(FieldError::new(
            "name",
            "A name cannot contain control characters or text-direction marks.",
        ));
    }
    Ok(name.to_owned())
}

pub(super) const INTERVAL_MESSAGE: &str = "The interval is 10 to 86400 seconds.";
pub(super) const RETRIES_MESSAGE: &str = "Retries are 0 to 10.";
pub(super) const RETRY_INTERVAL_MESSAGE: &str = "The retry interval is 5 to 3600 seconds.";
pub(super) const DEGRADED_MESSAGE: &str = "The Degraded limit is 1 to 600000 milliseconds.";

/// The database keeps the same bounds as a backstop but names only the first it meets.
pub(super) fn setting_bounds(
    interval_seconds: i32,
    retries: i32,
    retry_interval_seconds: i32,
    degraded_threshold_ms: i32,
) -> Vec<FieldError> {
    [
        (
            "intervalSeconds",
            10..=86_400,
            interval_seconds,
            INTERVAL_MESSAGE,
        ),
        ("retries", 0..=10, retries, RETRIES_MESSAGE),
        (
            "retryIntervalSeconds",
            5..=3_600,
            retry_interval_seconds,
            RETRY_INTERVAL_MESSAGE,
        ),
        (
            "degradedThresholdMs",
            1..=600_000,
            degraded_threshold_ms,
            DEGRADED_MESSAGE,
        ),
    ]
    .into_iter()
    .filter(|(_, bounds, value, _)| !bounds.contains(value))
    .map(|(field, _, _, message)| FieldError::new(field, message))
    .collect()
}

pub(super) fn config_error(error: ConfigError) -> FieldError {
    FieldError::new(error.field, format!("This value {}.", error.reason))
}

/// `stored` is the check's current setup, used only for empty secrets.
pub(super) fn prepare(definition: Definition, stored: Stored) -> Result<Prepared, FieldError> {
    match definition {
        Definition::Config(CheckConfig::Http(_)) => Err(FieldError::new(
            "config",
            "Give an HTTP check with its headers and authentication.",
        )),
        Definition::Config(CheckConfig::Push) => {
            Err(FieldError::new("config", "A push check needs a token."))
        }
        Definition::Config(config) => {
            config.validate().map_err(config_error)?;
            Ok(Prepared {
                config,
                secrets: None,
                push_token: None,
            })
        }
        Definition::Http(input) => {
            let stored = match stored {
                Stored::Http(stored) => Some(stored),
                _ => None,
            };
            let (config, secrets) = keep_stored(input, stored)?.split().map_err(config_error)?;
            Ok(Prepared {
                config: CheckConfig::Http(config),
                secrets: (!secrets.is_empty()).then(|| Sealable::Http(secrets)),
                push_token: None,
            })
        }
        Definition::Snmp(input) => {
            let (config, given) = input.split().map_err(config_error)?;
            let stored = match stored {
                Stored::Snmp(stored) => Some(stored),
                _ => None,
            };
            let secrets = keep_snmp(&config, given, stored)?;
            Ok(Prepared {
                config: CheckConfig::Snmp(config),
                secrets: Some(Sealable::Snmp(secrets)),
                push_token: None,
            })
        }
        Definition::Push(token) => Ok(Prepared {
            config: CheckConfig::Push,
            secrets: None,
            push_token: Some(token),
        }),
    }
}

pub(super) fn needs_stored(input: &HttpInput) -> bool {
    let empty_header = input
        .headers
        .iter()
        .any(|(name, value)| is_secret_header(name) && value.expose().is_empty());
    let empty_credential = match &input.credentials {
        HttpCredentials::None => false,
        HttpCredentials::Basic { password, .. } => password.expose().is_empty(),
        HttpCredentials::Bearer(token) => token.expose().is_empty(),
    };
    empty_header || empty_credential
}

pub(super) fn snmp_needs_stored(input: &SnmpInput) -> bool {
    let given = |secret: &Option<crate::fleet::credentials::Secret>| {
        secret.as_ref().is_some_and(|s| !s.expose().is_empty())
    };
    let credentials = &input.credentials;
    match input.config.version {
        SnmpVersion::V2c => !given(&credentials.community),
        SnmpVersion::V3 => {
            !given(&credentials.auth_password) || !given(&credentials.privacy_password)
        }
    }
}

/// Stored secrets fill a gap only for the same version and user name (SEC-005).
/// `stored` holds only secrets that opened for the device's current host (SEC-010).
fn keep_snmp(
    config: &SnmpConfig,
    given: SnmpCredentials,
    stored: Option<StoredSnmp>,
) -> Result<SnmpSecrets, FieldError> {
    let kept = stored
        .filter(|(kept, _)| kept.version == config.version && kept.username == config.username)
        .map(|(_, secrets)| secrets);
    let missing = |field: &'static str, what: &str| {
        FieldError::new(
            field,
            format!(
                "Enter the {what}: it is kept only for the same device host, version and user name."
            ),
        )
    };
    match (config.version, kept) {
        (SnmpVersion::V2c, kept) => {
            let stored = match kept {
                Some(SnmpSecrets::Community(community)) => Some(community),
                _ => None,
            };
            let community = given.community.or(stored);
            community
                .map(SnmpSecrets::Community)
                .ok_or_else(|| missing("community", "community"))
        }
        (SnmpVersion::V3, kept) => {
            let (stored_auth, stored_privacy) = match kept {
                Some(SnmpSecrets::User { auth, privacy }) => (Some(auth), Some(privacy)),
                _ => (None, None),
            };
            let auth = given.auth_password.or(stored_auth);
            let privacy = given.privacy_password.or(stored_privacy);
            let auth = auth.ok_or_else(|| missing("authPassword", "authentication password"))?;
            let privacy = privacy.ok_or_else(|| missing("privacyPassword", "privacy password"))?;
            Ok(SnmpSecrets::User { auth, privacy })
        }
    }
}

pub(super) fn origin(url: &str) -> Option<String> {
    let url = reqwest::Url::parse(url).ok()?;
    let host = url.host_str()?;
    let port = url.port_or_known_default()?;
    Some(format!("{}://{host}:{port}", url.scheme()))
}

fn secret_names<'a>(names: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut names: Vec<String> = names.map(str::to_ascii_lowercase).collect();
    names.sort_unstable();
    names
}

/// Whether stored secrets may go where the input sends them (SEC-005).
fn same_destination(input: &HttpInput, stored: &HttpConfig) -> bool {
    let origin_kept = origin(&input.url).is_some_and(|o| Some(o) == origin(&stored.url));
    let auth_kept = match (&input.credentials, &stored.auth) {
        (HttpCredentials::None, HttpAuth::None)
        | (HttpCredentials::Bearer(_), HttpAuth::Bearer) => true,
        (HttpCredentials::Basic { username, .. }, HttpAuth::Basic { username: kept }) => {
            username == kept
        }
        _ => false,
    };
    let tls_kept = input.verify_tls || !stored.verify_tls;
    let given = input
        .headers
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|n| is_secret_header(n));
    let kept = stored
        .headers
        .iter()
        .filter(|h| h.value.is_none())
        .map(|h| h.name.as_str());
    origin_kept && auth_kept && tls_kept && secret_names(given) == secret_names(kept)
}

/// The n-th empty secret header of a name takes the n-th stored value of that name.
fn keep_stored(mut input: HttpInput, stored: Option<StoredHttp>) -> Result<HttpInput, FieldError> {
    if !needs_stored(&input) {
        return Ok(input);
    }
    if stored
        .as_ref()
        .is_some_and(|(config, _)| !same_destination(&input, config))
    {
        return Err(FieldError::new(
            "config",
            "Enter every secret again: the address, the authentication or the secret headers changed.",
        ));
    }
    let (config, secrets) = stored.unwrap_or_else(|| (HttpConfig::new(""), HttpSecrets::default()));
    let HttpSecrets {
        password,
        bearer_token,
        headers,
    } = secrets;
    let mut kept: Vec<(String, Option<Secret>)> = config
        .headers
        .into_iter()
        .filter(|header| header.value.is_none())
        .map(|header| header.name)
        .zip(headers.into_iter().map(Some))
        .collect();
    let mut seen: Vec<String> = Vec::new();
    for (name, value) in &mut input.headers {
        if !is_secret_header(name) {
            continue;
        }
        let position = seen.iter().filter(|n| n.eq_ignore_ascii_case(name)).count();
        seen.push(name.clone());
        if value.expose().is_empty() {
            *value = kept
                .iter_mut()
                .filter(|(stored_name, _)| stored_name.eq_ignore_ascii_case(name))
                .nth(position)
                .and_then(|(_, stored_value)| stored_value.take())
                .ok_or_else(|| FieldError::new("headers", "Enter the value of this header."))?;
        }
    }
    let missing = || FieldError::new("auth", "Enter the password or token.");
    input.credentials = match input.credentials {
        HttpCredentials::Basic {
            username,
            password: given,
        } if given.expose().is_empty() => HttpCredentials::Basic {
            username,
            password: password.ok_or_else(missing)?,
        },
        HttpCredentials::Bearer(given) if given.expose().is_empty() => {
            HttpCredentials::Bearer(bearer_token.ok_or_else(missing)?)
        }
        other => other,
    };
    Ok(input)
}
