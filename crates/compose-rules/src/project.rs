use crate::{Node, Refusal, Rule, Settings, declared, forms::is_name};

const MOST_SERVICES: usize = 32;
const MOST_NETWORKS: usize = 16;
const MOST_VOLUMES: usize = 64;

pub(crate) struct Names<'a> {
    pub services: Vec<&'a str>,
    pub volumes: Vec<&'a str>,
    pub networks: Vec<&'a str>,
    pub configs: Vec<&'a str>,
}

fn names<'a>(document: &'a Node, key: &str, most: usize) -> Result<Vec<&'a str>, Refusal> {
    let entries = document.get(key).and_then(Node::entries).unwrap_or(&[]);
    if entries.len() > most {
        return Err(Refusal::top(Rule::TooLarge));
    }
    entries
        .iter()
        .map(|(name, _)| {
            is_name(name)
                .then_some(name.as_str())
                .ok_or(Refusal::top(Rule::NameNotPermitted))
        })
        .collect()
}

fn network(node: &Node) -> Result<(), Refusal> {
    let wrong = Refusal::top(Rule::ValueNotPermitted);
    match node.get("driver") {
        None | Some(Node::Null) => {}
        Some(driver) if driver.text() == Some("bridge") => {}
        Some(_) => return Err(wrong),
    }
    match node.get("internal").and_then(Node::text) {
        None | Some("true" | "false") => Ok(()),
        Some(_) => Err(wrong),
    }
}

/// Applies the rules of the values to a file whose keys are permitted.
pub(crate) fn checked(document: &Node, settings: Option<&Settings>) -> Result<(), Refusal> {
    let all = Names {
        services: names(document, "services", MOST_SERVICES)?,
        volumes: names(document, "volumes", MOST_VOLUMES)?,
        networks: names(document, "networks", MOST_NETWORKS)?,
        configs: names(document, "configs", usize::MAX)?,
    };
    if all.services.is_empty() {
        return Err(Refusal::top(Rule::ValueNotPermitted));
    }
    for (_, node) in document
        .get("networks")
        .and_then(Node::entries)
        .unwrap_or(&[])
    {
        network(node)?;
    }
    for (_, node) in document
        .get("configs")
        .and_then(Node::entries)
        .unwrap_or(&[])
    {
        if node.get("content").and_then(Node::text).is_none() {
            return Err(Refusal::top(Rule::ValueNotPermitted));
        }
    }
    let services = document
        .get("services")
        .and_then(Node::entries)
        .unwrap_or(&[]);
    for (position, (_, node)) in services.iter().enumerate() {
        declared::checked(node, position, &all, settings)?;
    }
    Ok(())
}
