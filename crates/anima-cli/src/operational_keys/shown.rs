//! How a key is shown: a table of its metadata, or the K1 object with `--json`. No field of
//! K1 holds a value, so nothing here can show one.

use serde_json::Value;

use super::documents::{SecretList, SecretShow, secret_list, secret_show};
use crate::failure::{Failure, Hint};
use crate::invocation::Invocation;
use crate::output::Table;

/// # Errors
/// [`Failure`] for a refusal or a missing token.
pub async fn list(invocation: &Invocation) -> Result<(), Failure> {
    let hub = invocation.hub()?;
    let answer = hub.run::<SecretList>(secret_list::Variables).await?;
    invocation.output.data(&answer.operational_keys, |keys| {
        let mut table = Table::new(vec![
            "NAME",
            "PROVIDER",
            "DUE",
            "ACTIVE",
            "CONSUMERS",
            "ROTATED",
        ]);
        for key in keys {
            let key = as_json(key);
            table.row(vec![
                text(&key, "name").into(),
                text(&key, "provider").into(),
                text(&key, "due").into(),
                active(&key).into(),
                count(&key, "consumers").into(),
                text(&key, "lastRotatedAt").into(),
            ]);
        }
        table
    })
}

/// # Errors
/// [`Failure::NotFound`] for an unknown name; [`Failure`] for a refusal.
pub async fn show(invocation: &Invocation, name: String) -> Result<(), Failure> {
    let hub = invocation.hub()?;
    let variables = secret_show::Variables { name: name.clone() };
    let answer = hub.run::<SecretShow>(variables).await?;
    let key = answer.operational_key.ok_or_else(|| not_found(&name))?;
    invocation.output.data(&key, |key| record(&as_json(key)))
}

#[must_use]
pub fn not_found(name: &str) -> Failure {
    Failure::NotFound {
        message: format!("There is no operational key named {name}."),
        hint: Some(Hint::run("anima secret list")),
    }
}

pub fn as_json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// The metadata of one key, its versions and its consumers.
#[must_use]
pub fn record(key: &Value) -> Table {
    let mut table = Table::record();
    for (label, field) in [
        ("Name", "name"),
        ("Provider", "provider"),
        ("Purpose", "purpose"),
        ("Due", "due"),
        ("Rotation days", "rotationDays"),
        ("Expires", "expiresAt"),
        ("Provider link", "providerLink"),
        ("Last rotated", "lastRotatedAt"),
        ("Rotated by", "lastRotatedBy"),
        ("Instructions", "instructions"),
    ] {
        table.pair(label, text(key, field));
    }
    table.pair("Active version", active(key));
    for version in list_of(key, "versions") {
        table.pair(
            &format!("Version {}", text(version, "number")),
            format!(
                "{} since {}{}",
                text(version, "state"),
                text(version, "createdAt"),
                if version["rotatedAtProvider"] == Value::Bool(true) {
                    ", changed at the provider"
                } else {
                    ""
                }
            ),
        );
    }
    for consumer in list_of(key, "consumers") {
        table.pair("Consumer", consumer_line(consumer));
    }
    table
}

/// A device and its workload, with the version its daemon held at its last report.
#[must_use]
pub fn consumer_line(consumer: &Value) -> String {
    format!(
        "{} {} (holds {})",
        text(consumer, "deviceId"),
        text(consumer, "workload"),
        match consumer["heldVersion"].as_i64() {
            Some(version) => format!("version {version}"),
            None => "no version".to_owned(),
        }
    )
}

pub fn list_of<'a>(value: &'a Value, field: &str) -> &'a [Value] {
    value[field].as_array().map_or(&[], Vec::as_slice)
}

fn text(value: &Value, field: &str) -> String {
    match &value[field] {
        Value::Null => "-".to_owned(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn active(key: &Value) -> String {
    key["activeVersion"]
        .as_i64()
        .map_or_else(|| "none".to_owned(), |version| version.to_string())
}

fn count(value: &Value, field: &str) -> String {
    list_of(value, field).len().to_string()
}
