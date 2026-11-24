//! The variables of a file: `${NAME}` and `${NAME:-text}`, filled from the environment file of
//! the item and from nothing else (K10 Section 6.4.4, step 3).

use std::collections::BTreeMap;

use crate::{Node, Refusal, Rule};

const REFUSED: Refusal = Refusal::top(Rule::VariableNotPermitted);

fn is_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
}

/// The names and the values of an environment file of `NAME=value` lines, with no rule for quotes.
pub(crate) fn environment(text: &str) -> Result<BTreeMap<String, String>, Refusal> {
    let mut values = BTreeMap::new();
    // Lines are cut at the line feed only, so a carriage return is refused.
    for line in text.split('\n') {
        if line.contains(['\r', '\0']) {
            return Err(REFUSED);
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, value) = line.split_once('=').ok_or(REFUSED)?;
        if !is_name(name) || values.insert(name.to_owned(), value.to_owned()).is_some() {
            return Err(REFUSED);
        }
    }
    Ok(values)
}

/// `$$` is one `$` and each other bare `$` is refused; the result is not filled a second time.
fn fill(text: &str, values: &BTreeMap<String, String>) -> Result<String, Refusal> {
    let mut filled = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('$') {
        filled.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        if let Some(after) = after.strip_prefix('$') {
            filled.push('$');
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
        let value = values.get(name).map(String::as_str).or(default);
        filled.push_str(value.ok_or(REFUSED)?);
        rest = after;
    }
    filled.push_str(rest);
    Ok(filled)
}

/// A key is never filled: a key that holds `$` is refused.
pub(crate) fn filled(node: Node, values: &BTreeMap<String, String>) -> Result<Node, Refusal> {
    Ok(match node {
        Node::Null => Node::Null,
        Node::Text(text) => Node::Text(fill(&text, values)?),
        Node::List(items) => Node::List(
            items
                .into_iter()
                .map(|item| filled(item, values))
                .collect::<Result<_, _>>()?,
        ),
        Node::Map(entries) => Node::Map(
            entries
                .into_iter()
                .map(|(key, value)| {
                    if key.contains('$') {
                        return Err(REFUSED);
                    }
                    Ok((key, filled(value, values)?))
                })
                .collect::<Result<_, _>>()?,
        ),
    })
}
