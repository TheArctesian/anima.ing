//! Inventory startup evidence under the selected Podman account.
use std::{collections::BTreeSet, path::Path};

use nix::unistd::User;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Error, Result, users};

const MAX_ENTRIES: usize = 128;
const QUADLETS: &str = concat!(
    r#"{{range .}}{"name":{{json .Name}},"path":{{json .Path}},"unit":{{json .UnitName}}}"#,
    "{{println}}{{end}}",
);

#[derive(Serialize)]
pub(super) struct Inventory {
    quadlets: Option<Vec<Quadlet>>,
    units: Option<Vec<Unit>>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Quadlet {
    name: String,
    path: String,
    unit: String,
}

#[derive(Serialize)]
struct Unit {
    name: String,
    enablement: String,
    evidence: Evidence,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum Evidence {
    Quadlet,
    ContainerLabel,
    NamePattern,
}

pub(super) async fn inventory(user: &User, containers: &[Value]) -> Inventory {
    let (quadlets, units) = tokio::join!(quadlets(user), list_units(user));
    let quadlets = quadlets.ok();
    let units = units
        .and_then(|lines| select_units(&lines, quadlets.as_deref().unwrap_or_default(), containers))
        .ok();
    Inventory { quadlets, units }
}

async fn quadlets(user: &User) -> Result<Vec<Quadlet>> {
    let output = users::run(
        user,
        "/run/current-system/sw/bin/podman",
        &["--remote=false", "quadlet", "list", "--format", QUADLETS],
        6,
    )
    .await?;
    let mut entries = Vec::new();
    for line in output.lines().filter(|line| !line.is_empty()) {
        let row: Quadlet = serde_json::from_str(line)?;
        let path = Path::new(&row.path);
        if entries.len() >= MAX_ENTRIES
            || row.path.len() > 2048
            || row.path.chars().any(char::is_control)
            || !path.is_absolute()
            || path.file_name().and_then(std::ffi::OsStr::to_str) != Some(row.name.as_str())
            || (!row.unit.is_empty() && !service_name(&row.unit))
        {
            return Err(unavailable());
        }
        entries.push(row);
    }
    entries.sort_unstable_by(|a, b| a.name.cmp(&b.name).then(a.path.cmp(&b.path)));
    Ok(entries)
}

async fn list_units(user: &User) -> Result<String> {
    users::run(
        user,
        "/run/current-system/sw/bin/systemctl",
        &[
            if user.uid.is_root() {
                "--system"
            } else {
                "--user"
            },
            "--no-pager",
            "--no-legend",
            "--plain",
            "--full",
            "--type=service,timer",
            "list-unit-files",
        ],
        6,
    )
    .await
}

fn select_units(output: &str, quadlets: &[Quadlet], containers: &[Value]) -> Result<Vec<Unit>> {
    let generated: BTreeSet<_> = quadlets
        .iter()
        .map(|quadlet| quadlet.unit.as_str())
        .collect();
    let reported: BTreeSet<_> = containers
        .iter()
        .filter_map(|container| container["systemdUnitHint"].as_str())
        .filter(|name| service_name(name))
        .collect();
    let mut entries = Vec::new();
    for line in output.lines().filter(|line| !line.is_empty()) {
        let mut columns = line.split_ascii_whitespace();
        let name = columns
            .next()
            .filter(|name| unit_name(name))
            .ok_or_else(unavailable)?;
        let enablement = columns
            .next()
            .filter(|state| {
                state.len() <= 32
                    && state
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
            })
            .ok_or_else(unavailable)?;
        let evidence = if generated.contains(name) {
            Evidence::Quadlet
        } else if reported.contains(name) {
            Evidence::ContainerLabel
        } else if ["container-", "pod-", "podman-"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
        {
            Evidence::NamePattern
        } else {
            continue;
        };
        if entries.len() >= MAX_ENTRIES {
            return Err(unavailable());
        }
        entries.push(Unit {
            name: name.to_owned(),
            enablement: enablement.to_owned(),
            evidence,
        });
    }
    Ok(entries)
}

pub(super) fn service_name(name: &str) -> bool {
    name.ends_with(".service") && unit_name(name)
}

fn unit_name(name: &str) -> bool {
    let stem = name
        .strip_suffix(".service")
        .or_else(|| name.strip_suffix(".timer"));
    stem.is_some_and(|stem| !stem.is_empty())
        && name.len() <= 255
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_.:@-\\".contains(&byte))
}

fn unavailable() -> Error {
    Error::Refused("Startup inventory was incomplete or exceeded its limit.")
}
