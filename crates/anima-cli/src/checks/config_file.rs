//! A check's configuration comes only from a file: a secret is never an argument (CMD-025).

use std::path::Path;

use serde::de::DeserializeOwned;

use crate::failure::{Failure, Hint};
use crate::input::{self, MAX_DOCUMENT};
use crate::prompt::Prompter;

const KINDS: [&str; 12] = [
    "icmp",
    "tcp",
    "tls",
    "http",
    "dns",
    "push",
    "group",
    "docker",
    "snmp",
    "tailscalePing",
    "systemd",
    "domainExpiry",
];

/// # Errors
/// [`Failure::Input`] or [`Failure::Usage`]; the message names the position, never the content.
pub fn read<T: DeserializeOwned>(prompter: Prompter, path: &Path) -> Result<T, Failure> {
    let bytes = input::read(prompter, path, "the check configuration", MAX_DOCUMENT)?;
    let value: serde_json::Value = input::json(
        &bytes,
        "the check configuration",
        "anima check create --help",
    )?;
    check_shape(&value, path)?;
    serde_json::from_value(value).map_err(|_| Failure::Usage {
        message: format!(
            "A value in {} does not fit the check's settings; see CheckConfigInput in K1.",
            path.display()
        ),
        hint: Some(Hint::run("anima check create --help")),
    })
}

fn check_shape(value: &serde_json::Value, path: &Path) -> Result<(), Failure> {
    let usage = |message: String| Failure::Usage {
        message,
        hint: Some(Hint::run("anima check create --help")),
    };
    let Some(object) = value.as_object() else {
        return Err(usage(format!(
            "{} must hold a JSON object.",
            path.display()
        )));
    };
    if object.len() != 1 {
        return Err(usage(format!(
            "{} must have exactly one key, the kind: one of {}.",
            path.display(),
            KINDS.join(", ")
        )));
    }
    let kind = object.keys().next().map_or("", String::as_str);
    if !KINDS.contains(&kind) {
        return Err(usage(format!(
            "{} has the key {kind}; the kind must be one of {}.",
            path.display(),
            KINDS.join(", ")
        )));
    }
    Ok(())
}
