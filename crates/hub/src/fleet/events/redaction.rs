//! Removes secret values from audit changes and event text before they are stored (LOG-007, D-3).
//! It over-matches (`tokenExpiresAt` is redacted) rather than let a secret through.

#[cfg(test)]
mod tests;

use chrono::DateTime;
use serde_json::{Map, Value};

pub(super) use ::redaction::redact_text;
use ::redaction::{REDACTED, Secrecy, normalise, secrecy};

/// Keys whose value names the field an object describes; `name` counts only beside `value`,
/// since a device may be called `secret-lab`.
const LABELS: [&str; 5] = ["field", "path", "key", "setting", "property"];

const HOLDER_METADATA: [&str; 11] = [
    "kind",
    "type",
    "username",
    "id",
    "fingerprint",
    "createdat",
    "updatedat",
    "rotatedat",
    "storedat",
    "expiresat",
    "lastusedat",
];

const AUTH_METHODS: [&str; 11] = [
    "password",
    "key",
    "publickey",
    "privatekey",
    "none",
    "token",
    "totp",
    "basic",
    "agent",
    "keyboardinteractive",
    "kerberos",
];

/// Replaces every secret in `value` with `"[redacted]"`, at any depth.
pub(super) fn redact(value: Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(redact_fields(fields)),
        Value::Array(values) => Value::Array(values.into_iter().map(redact).collect()),
        Value::String(text) => Value::String(redact_text(&text)),
        other => other,
    }
}

fn redact_fields(fields: Map<String, Value>) -> Map<String, Value> {
    let header = ["name", "value"]
        .iter()
        .all(|wanted| fields.keys().any(|key| normalise(key) == *wanted));
    let secret_label = fields
        .iter()
        .find(|(key, value)| {
            is_label(key, header) && value.as_str().is_some_and(label_names_secret)
        })
        .map(|(key, _)| key.clone());
    fields
        .into_iter()
        .map(|(key, value)| {
            let value = match &secret_label {
                Some(label) if key != *label && normalise(&key) != "op" => conceal(value),
                Some(_) => redact(value),
                None if normalise(&key) == "auth" => redact_auth(value),
                None => match secrecy(&key) {
                    Secrecy::Holder => redact_holder(value),
                    Secrecy::Secret => conceal(value),
                    Secrecy::Plain => redact(value),
                },
            };
            (key, value)
        })
        .collect()
}

fn redact_holder(value: Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| {
                    let value = match secrecy(&key) {
                        Secrecy::Secret => conceal(value),
                        _ if is_metadata(&key, &value) => redact(value),
                        _ => redact_holder(value),
                    };
                    (key, value)
                })
                .collect(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(redact_holder).collect()),
        scalar => conceal(scalar),
    }
}

fn is_metadata(key: &str, value: &Value) -> bool {
    match value {
        Value::Object(_) | Value::Array(_) => false,
        Value::String(text) if DateTime::parse_from_rfc3339(text).is_ok() => true,
        _ => HOLDER_METADATA.contains(&normalise(key).as_str()),
    }
}

fn redact_auth(value: Value) -> Value {
    match value {
        Value::String(method) if is_auth_method(&method) => Value::String(method),
        Value::Object(_) | Value::Array(_) => redact_holder(value),
        scalar => conceal(scalar),
    }
}

/// Both sides are normalised, which drops a plural `s` (`kerberos` gives `kerbero`).
fn is_auth_method(method: &str) -> bool {
    let method = normalise(method);
    AUTH_METHODS.iter().any(|known| normalise(known) == method)
}

fn conceal(value: Value) -> Value {
    if value.is_boolean() {
        value
    } else {
        Value::String(REDACTED.to_owned())
    }
}

fn is_label(key: &str, header: bool) -> bool {
    let key = normalise(key);
    LABELS.contains(&key.as_str()) || (header && key == "name")
}

fn label_names_secret(label: &str) -> bool {
    label
        .split(['/', '.'])
        .any(|segment| secrecy(segment) != Secrecy::Plain)
}
