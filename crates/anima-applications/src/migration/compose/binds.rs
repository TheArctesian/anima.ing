use std::collections::{BTreeMap, BTreeSet};

use compose_rules::{Node, Variables, forms::is_name};
use serde::Deserialize;
use zeroize::Zeroizing;

use super::{Error, document::field, fields};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mapping {
    source: String,
    bind: String,
}

pub(super) struct Binds {
    by_source: BTreeMap<String, String>,
    used: BTreeSet<String>,
}

impl Binds {
    pub(super) fn read(json: &str, installed: &[String]) -> Result<Self, Error> {
        let rows: Vec<Mapping> = serde_json::from_str(json).map_err(|_| Error::ComposeBinds)?;
        let mut by_source = BTreeMap::new();
        let mut bound = BTreeSet::new();
        for Mapping { source, bind } in rows {
            if source.is_empty()
                || source.len() > 4096
                || source.contains('$')
                || source.chars().any(char::is_control)
                || !installed.contains(&bind)
                || !bound.insert(bind.clone())
                || by_source.insert(source, bind).is_some()
            {
                return Err(Error::ComposeBinds);
            }
        }
        if bound.len() != installed.len() {
            return Err(Error::ComposeBinds);
        }
        Ok(Self {
            by_source,
            used: BTreeSet::new(),
        })
    }

    pub(super) fn rewrite(
        &mut self,
        mounts: &mut Node,
        named: &BTreeSet<String>,
        variables: &Variables<'_>,
    ) -> Result<(), Error> {
        let Node::List(mounts) = mounts else {
            return Err(Error::ComposeMount);
        };
        for mount in mounts {
            let original = Mount::read(mount)?;
            let source = original.resolve_source(named, variables)?;
            let (registered, bind) = self
                .by_source
                .get_key_value(source.as_str())
                .ok_or(Error::ComposeBinds)?;
            self.used.insert(registered.clone());
            *mount = Node::Map(vec![
                field("type", "bind"),
                field("source", &format!("./binds/{bind}")),
                field("target", original.target),
                field(
                    "read_only",
                    if original.read_only { "true" } else { "false" },
                ),
            ]);
        }
        Ok(())
    }

    pub(super) fn require_all_used(&self, named: &BTreeSet<String>) -> Result<(), Error> {
        if self.used.len() == self.by_source.len() && named.is_subset(&self.used) {
            Ok(())
        } else {
            Err(Error::ComposeBinds)
        }
    }
}

pub(super) fn declared(document: &[(String, Node)]) -> Result<BTreeSet<String>, Error> {
    let Some((_, volumes)) = document.iter().find(|(key, _)| key == "volumes") else {
        return Ok(BTreeSet::new());
    };
    let entries = volumes.entries().ok_or(Error::ComposeMount)?;
    entries
        .iter()
        .map(|(name, volume)| {
            let settings = volume.entries().ok_or(Error::ComposeMount)?;
            if !is_name(name)
                || !settings
                    .iter()
                    .all(|(key, value)| key == "driver" && value.text() == Some("local"))
            {
                return Err(Error::ComposeMount);
            }
            Ok(name.to_owned())
        })
        .collect()
}

struct Mount<'a> {
    kind: Option<&'a str>,
    source: &'a str,
    target: &'a str,
    read_only: bool,
}

impl<'a> Mount<'a> {
    fn read(node: &'a Node) -> Result<Self, Error> {
        match node {
            Node::Text(value) => Self::short(value),
            Node::Map(settings)
                if settings.iter().all(|(key, _)| {
                    matches!(key.as_str(), "type" | "source" | "target" | "read_only")
                }) =>
            {
                Self::long(node)
            }
            _ => Err(Error::ComposeMount),
        }
    }

    fn resolve_source(
        &self,
        named: &BTreeSet<String>,
        variables: &Variables<'_>,
    ) -> Result<Zeroizing<String>, Error> {
        let source = Zeroizing::new(variables.resolve(self.source)?);
        let path = is_path(&source);
        let kind = if let Some(kind) = self.kind {
            kind
        } else {
            let target = Zeroizing::new(variables.resolve(self.target)?);
            if source.contains(':') || target.contains(':') {
                return Err(Error::ComposeMount);
            }
            if path { "bind" } else { "volume" }
        };
        match kind {
            "bind" if path => Ok(source),
            "volume" if named.contains(source.as_str()) => Ok(source),
            _ => Err(Error::ComposeMount),
        }
    }

    fn short(value: &'a str) -> Result<Self, Error> {
        let (source, rest) = fields::first(value, ':').ok_or(Error::ComposeMount)?;
        let (target, mode) = fields::first(rest, ':').unwrap_or((rest, "rw"));
        let read_only = match mode {
            "rw" => false,
            "ro" => true,
            _ => return Err(Error::ComposeMount),
        };
        Ok(Self {
            kind: None,
            source,
            target,
            read_only,
        })
    }

    fn long(node: &'a Node) -> Result<Self, Error> {
        let text = |key| {
            node.get(key)
                .and_then(Node::text)
                .ok_or(Error::ComposeMount)
        };
        let read_only = node.get("read_only").map_or(Ok(false), |value| {
            value
                .text()
                .and_then(compose_rules::forms::truth)
                .ok_or(Error::ComposeMount)
        })?;
        Ok(Self {
            kind: Some(text("type")?),
            source: text("source")?,
            target: text("target")?,
            read_only,
        })
    }
}

fn is_path(text: &str) -> bool {
    text.starts_with('/') || text.starts_with("./") || text.starts_with("../")
}
