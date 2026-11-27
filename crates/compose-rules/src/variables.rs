//! The variables of a file: `${NAME}` and `${NAME:-text}`, filled from the environment file of
//! the item and from nothing else (K10 Section 6.4.4, step 3).

use std::collections::BTreeMap;

use crate::{LARGEST_ENVIRONMENT, Node, Refusal, Rule};

const EXPANDED_VALUE_BYTES: usize = 4 * 1024 * 1024;
const TOO_LARGE: Refusal = Refusal::top(Rule::TooLarge);
const REFUSED: Refusal = Refusal::top(Rule::VariableNotPermitted);

fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

/// The values of one explicit Compose environment file.
pub struct Variables<'a>(BTreeMap<&'a str, &'a str>);

impl<'a> Variables<'a> {
    /// Parse bounded `NAME=value` lines without consulting the process environment.
    ///
    /// # Errors
    /// The file exceeds the limit or contains malformed or duplicate entries.
    pub fn parse(text: &'a str) -> Result<Self, Refusal> {
        if text.len() > LARGEST_ENVIRONMENT {
            return Err(TOO_LARGE);
        }
        let mut values = BTreeMap::new();
        for line in text.split('\n') {
            if line.contains(['\r', '\0']) {
                return Err(REFUSED);
            }
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (name, value) = line.split_once('=').ok_or(REFUSED)?;
            if !is_name(name) || values.insert(name, value).is_some() {
                return Err(REFUSED);
            }
        }
        Ok(Self(values))
    }

    /// Resolve one scalar with the same interpolation rules as a Hosting file.
    ///
    /// # Errors
    /// Interpolation is invalid or the expanded value exceeds the bound.
    pub fn resolve(&self, template: &str) -> Result<String, Refusal> {
        let mut remaining = EXPANDED_VALUE_BYTES;
        fill(template, &self.0, &mut remaining)
    }
}

/// `$$` is one `$` and each other bare `$` is refused; the result is not filled a second time.
fn fill(
    text: &str,
    values: &BTreeMap<&str, &str>,
    remaining: &mut usize,
) -> Result<String, Refusal> {
    let mut filled = String::with_capacity(text.len().min(*remaining));
    let mut rest = text;
    while let Some(at) = rest.find('$') {
        append(&mut filled, &rest[..at], remaining)?;
        let after = &rest[at + 1..];
        if let Some(after) = after.strip_prefix('$') {
            append(&mut filled, "$", remaining)?;
            rest = after;
            continue;
        }
        let inner = after.strip_prefix('{').ok_or(REFUSED)?;
        let (variable, after) = inner.split_once('}').ok_or(REFUSED)?;
        // The text of a default has no `$`: it is never a second variable.
        if variable.contains('$') {
            return Err(REFUSED);
        }
        let (name, default) = match variable.split_once(":-") {
            Some((name, default)) => (name, Some(default)),
            None => (variable, None),
        };
        if !is_name(name) {
            return Err(REFUSED);
        }
        let value = values.get(name).copied().or(default);
        append(&mut filled, value.ok_or(REFUSED)?, remaining)?;
        rest = after;
    }
    append(&mut filled, rest, remaining)?;
    Ok(filled)
}

fn append(output: &mut String, value: &str, remaining: &mut usize) -> Result<(), Refusal> {
    *remaining = remaining.checked_sub(value.len()).ok_or(TOO_LARGE)?;
    output.push_str(value);
    Ok(())
}

/// A key is never filled: a key that holds `$` is refused.
pub(crate) fn filled(node: Node, values: &Variables<'_>) -> Result<Node, Refusal> {
    let mut remaining = EXPANDED_VALUE_BYTES;
    fill_node(node, &values.0, &mut remaining)
}

fn fill_node(
    node: Node,
    values: &BTreeMap<&str, &str>,
    remaining: &mut usize,
) -> Result<Node, Refusal> {
    Ok(match node {
        Node::Null => Node::Null,
        Node::Text(text) => Node::Text(fill(&text, values, remaining)?),
        Node::List(items) => Node::List(
            items
                .into_iter()
                .map(|item| fill_node(item, values, remaining))
                .collect::<Result<_, _>>()?,
        ),
        Node::Map(entries) => Node::Map(
            entries
                .into_iter()
                .map(|(key, value)| {
                    if key.contains('$') {
                        return Err(REFUSED);
                    }
                    Ok((key, fill_node(value, values, remaining)?))
                })
                .collect::<Result<_, _>>()?,
        ),
    })
}
