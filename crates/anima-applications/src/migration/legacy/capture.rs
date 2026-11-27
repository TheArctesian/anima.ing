//! Bind a stopped or removed source to the identity it had before shutdown.
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

use nix::unistd::User;
use serde::{Deserialize, Serialize};

use crate::forms::{is_lower_hex, is_path};

use super::{Error, Manifest, account, graph_root, launchers, podman, stopped_containers};

const CAPTURE: &str = concat!(
    r#"{"id":{{json .ID}},"name":{{json .Name}},"autoRemove":{{json .HostConfig.AutoRemove}},"#,
    r#""unit":{{json (index .Config.Labels "PODMAN_SYSTEMD_UNIT")}}}"#,
);
const LIST: &str = r#"{"id":{{json .ID}},"name":{{json .Names}}}"#;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::migration) struct Snapshot {
    graph_root: String,
    containers: Vec<Container>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Container {
    id: String,
    name: String,
    auto_remove: bool,
    unit: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Listed {
    id: String,
    name: String,
}

pub(in crate::migration) fn capture(
    manifest: &Manifest,
    deadline: Instant,
) -> Result<Snapshot, Error> {
    let user = account(manifest.podman_uid)?;
    let root = graph_root(&user, deadline)?;
    let mut args = vec!["container", "inspect", "--format", CAPTURE, "--"];
    args.extend(manifest.podman_containers.iter().map(String::as_str));
    let output = podman(&user, &args, deadline)?;
    let mut containers: Vec<Container> = rows(&output).ok_or(Error::SourceCapture)?;
    for container in &mut containers {
        if container.unit.as_deref() == Some("") {
            container.unit = None;
        }
    }
    containers.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    if graph_root(&user, deadline)? != root {
        return Err(Error::SourceCapture);
    }
    Ok(Snapshot {
        graph_root: root,
        containers,
    })
}

impl Snapshot {
    pub(in crate::migration) fn valid(&self, expected: &[String]) -> bool {
        is_path(&self.graph_root)
            && self
                .containers
                .iter()
                .any(|container| container.auto_remove)
            && self.containers.len() == expected.len()
            && self.containers.iter().all(Container::valid)
            && distinct(
                self.containers
                    .iter()
                    .map(|container| container.name.as_str()),
            )
            && self
                .containers
                .iter()
                .map(|container| container.id.as_str())
                .collect::<BTreeSet<_>>()
                == expected.iter().map(String::as_str).collect()
    }

    pub(super) fn stopped(&self, user: &User, deadline: Instant) -> Result<String, Error> {
        let mut units: BTreeSet<_> = self
            .containers
            .iter()
            .filter_map(|container| container.unit.clone())
            .collect();
        launchers::require_fenced(user, &units, deadline)?;
        let present = self.retained(&inventory(user, deadline)?)?;
        if !present.is_empty() {
            units.extend(stopped_containers(user, &present, deadline)?);
            launchers::require_fenced(user, &units, deadline)?;
        }
        let root = graph_root(user, deadline)?;
        if self.graph_root != root {
            return Err(Error::SourceActive);
        }
        Ok(root)
    }

    fn retained(&self, present: &BTreeMap<String, String>) -> Result<Vec<String>, Error> {
        let mut retained = Vec::new();
        for container in &self.containers {
            let found = present.get(&container.name);
            if found == Some(&container.id) {
                retained.push(container.id.clone());
            } else if found.is_some()
                || !container.auto_remove
                || present.values().any(|id| *id == container.id)
            {
                return Err(Error::SourceActive);
            }
        }
        retained.sort_unstable();
        Ok(retained)
    }
}

impl Container {
    fn valid(&self) -> bool {
        is_lower_hex(&self.id, 64)
            && valid_name(&self.name)
            && self.unit.as_deref().is_none_or(launchers::service_name)
            && (!self.auto_remove || self.unit.is_some())
    }
}

fn inventory(user: &User, deadline: Instant) -> Result<BTreeMap<String, String>, Error> {
    let output = podman(
        user,
        &["ps", "--all", "--no-trunc", "--format", LIST],
        deadline,
    )?;
    let listed: Vec<Listed> = rows(&output).ok_or(Error::SourceActive)?;
    let mut found = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for Listed { id, name } in listed {
        if !is_lower_hex(&id, 64)
            || !valid_name(&name)
            || !ids.insert(id.clone())
            || found.insert(name, id).is_some()
        {
            return Err(Error::SourceActive);
        }
    }
    Ok(found)
}

fn rows<T: serde::de::DeserializeOwned>(output: &[u8]) -> Option<Vec<T>> {
    std::str::from_utf8(output)
        .ok()?
        .lines()
        .map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn distinct<'a>(names: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = BTreeSet::new();
    names.into_iter().all(|name| seen.insert(name))
}

fn valid_name(name: &str) -> bool {
    name.len() <= 128
        && name
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.-".contains(&byte))
}
