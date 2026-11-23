//! No file of a recipe holds a secret value: the install makes each secret.

use compose_rules::Node;

use crate::{
    Code,
    privileges::{commands, environment},
};

pub(crate) const GENERATED: &str = "# generated at install";

const SECRET_PARTS: [&str; 2] = ["PASS", "KEY"];
/// Texts that mark a secret anywhere in a name (SF-225).
const SECRET_TEXTS: [&str; 7] = [
    "PASSWORD",
    "PASSWD",
    "SECRET",
    "TOKEN",
    "APIKEY",
    "CREDENTIAL",
    "PRIVATE_KEY",
];

pub(crate) fn is_secret_name(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    name.split('_').any(|part| SECRET_PARTS.contains(&part))
        || SECRET_TEXTS.iter().any(|text| name.contains(text))
}

fn is_one_variable(value: &str) -> bool {
    value
        .strip_prefix("${")
        .and_then(|rest| rest.strip_suffix('}'))
        .is_some_and(|name| {
            !name.is_empty()
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
        })
}

/// A secret name with a literal or a `:-` default, in the environment or the commands (SF-251).
pub(crate) fn secret_in_file(services: &[(String, Node)]) -> bool {
    services.iter().any(|(_, service)| {
        environment(service)
            .into_iter()
            .any(|(name, value)| is_secret_name(name) && !is_one_variable(value))
            || commands::secret_literal(service, is_secret_name)
    })
}

struct Line<'a> {
    name: &'a str,
    value: &'a str,
    marked: bool,
}

fn lines(text: &str) -> Vec<Line<'_>> {
    let mut above = "";
    let mut found = Vec::new();
    for line in text.lines() {
        if let Some((name, value)) = line.split_once('=')
            && !line.starts_with('#')
        {
            found.push(Line {
                name,
                value,
                marked: above == GENERATED,
            });
        }
        above = line;
    }
    found
}

pub(crate) fn check(text: &str, generated: &[String]) -> Vec<Code> {
    let found = lines(text);
    let mut codes = Vec::new();
    let secret = found.iter().any(|line| {
        !line.value.is_empty()
            && (is_secret_name(line.name) || generated.iter().any(|name| name == line.name))
    });
    if secret {
        codes.push(Code::SecretInExample);
    }
    let each_marked = generated
        .iter()
        .all(|name| found.iter().any(|line| line.name == name && line.marked));
    let each_listed = found
        .iter()
        .filter(|line| line.marked)
        .all(|line| generated.iter().any(|name| name == line.name));
    if !each_marked || !each_listed {
        codes.push(Code::GeneratedNotMarked);
    }
    codes
}
