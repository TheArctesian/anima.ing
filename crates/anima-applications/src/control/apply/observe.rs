//! Collect only the engine and filesystem facts used by a managed Compose apply.
use std::{collections::BTreeMap, io, time::Instant};

use serde::Deserialize;

use crate::{
    Container, Inventory, Paths,
    execution::{Fault, check_socket, process},
    forms::{is_engine_name, is_lower_hex, is_path},
    plan::Project,
    settings::Machine,
};

const CONTAINERS: usize = 128;
const INSPECT_FIELDS: &str = concat!(
    r#"{"id":{{json .Id}},"name":{{json .Name}},"running":{{and .State.Running (not .State.Paused) (not .State.Restarting)}},"#,
    r#""project":{{json (index .Config.Labels "com.docker.compose.project")}},"#,
    r#""folder":{{json (index .Config.Labels "com.docker.compose.project.working_dir")}},"#,
    r#""service":{{json (index .Config.Labels "com.docker.compose.service")}},"#,
    r#""health":{{if .State.Health}}{{json .State.Health.Status}}{{else}}null{{end}}"#,
);
const CUTOVER_FIELDS: &str = concat!(
    r#","final_form":{{json (index .Config.Labels "com.docker.compose.project.config_files")}},"#,
    r#""config_hash":{{json (index .Config.Labels "com.docker.compose.config-hash")}},"#,
    r#""image":{{json .Config.Image}},"mounts":{{if .Mounts}}{{json .Mounts}}{{else}}[]{{end}}"#,
);
const ENGINE: &str = concat!(
    r#"{"root":{{json .DockerRootDir}},"memory":{{json .MemTotal}},"#,
    r#""processors":{{json .NCPU}}}"#,
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Engine {
    root: String,
    memory: u64,
    processors: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProjectContainer {
    id: String,
    name: String,
    pub running: bool,
    project: String,
    pub folder: String,
    pub service: String,
    pub health: Option<String>,
    pub final_form: Option<String>,
    pub config_hash: Option<String>,
    pub image: Option<String>,
    #[serde(default)]
    pub mounts: Vec<Mount>,
}

#[derive(Deserialize)]
pub(super) struct Mount {
    #[serde(rename = "Type")]
    pub kind: String,
    #[serde(rename = "Source", default)]
    pub source: String,
    #[serde(rename = "Destination")]
    pub destination: String,
    #[serde(rename = "RW")]
    pub writable: bool,
}

impl ProjectContainer {
    fn into_inventory(self) -> Container {
        Container {
            id: self.id,
            name: self.name,
            project: Some(self.project),
            folder: Some(self.folder),
            running: self.running,
            restarts: false,
        }
    }
}

pub(super) fn machine(paths: &Paths, deadline: Instant) -> Result<Machine, Fault> {
    check_socket(paths)?;
    let command = paths.docker(&["info", "--format", ENGINE], &[]);
    let info = process::capture_bounded(paths, &command, deadline, 8192)?;
    let engine: Engine = serde_json::from_slice(&info).map_err(|_| invalid())?;
    if !is_path(&engine.root) || engine.memory == 0 || engine.processors == 0 {
        return Err(invalid());
    }
    let space = rustix::fs::statvfs(&engine.root)?;
    Ok(Machine {
        memory_bytes: engine.memory,
        processors: engine.processors,
        free_disk_bytes: space.f_frsize.saturating_mul(space.f_bavail),
    })
}

pub(super) fn inventory(
    paths: &Paths,
    project: &str,
    deadline: Instant,
) -> Result<Inventory, Fault> {
    Ok(Inventory {
        containers: containers(paths, project, deadline)?
            .into_iter()
            .map(ProjectContainer::into_inventory)
            .collect(),
        ..Inventory::default()
    })
}

pub(super) fn containers(
    paths: &Paths,
    project: &str,
    deadline: Instant,
) -> Result<Vec<ProjectContainer>, Fault> {
    project_containers(paths, project, ("", 8192), deadline)
}

pub(super) fn cutover_containers(
    paths: &Paths,
    project: &str,
    deadline: Instant,
) -> Result<Vec<ProjectContainer>, Fault> {
    project_containers(paths, project, (CUTOVER_FIELDS, 64 * 1024), deadline)
}

fn project_containers(
    paths: &Paths,
    project: &str,
    (fields, max_bytes): (&str, usize),
    deadline: Instant,
) -> Result<Vec<ProjectContainer>, Fault> {
    let ids = ids(paths, project, deadline)?;
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    let format = format!("{INSPECT_FIELDS}{fields}}}");
    inspect(paths, project, (&ids, &format, max_bytes), deadline)
}

fn ids(paths: &Paths, project: &str, deadline: Instant) -> Result<Vec<String>, Fault> {
    let filter = format!("label=com.docker.compose.project={project}");
    let command = paths.docker(
        &[
            "container",
            "ls",
            "--all",
            "--no-trunc",
            "--quiet",
            "--filter",
            &filter,
        ],
        &[],
    );
    check_socket(paths)?;
    let output = process::capture_bounded(paths, &command, deadline, CONTAINERS * 65)?;
    std::str::from_utf8(&output)
        .map_err(|_| invalid())?
        .lines()
        .map(|id| {
            is_lower_hex(id, 64)
                .then(|| id.to_owned())
                .ok_or_else(invalid)
        })
        .collect()
}

fn inspect(
    paths: &Paths,
    project: &str,
    (ids, format, max_bytes): (&[String], &str, usize),
    deadline: Instant,
) -> Result<Vec<ProjectContainer>, Fault> {
    check_socket(paths)?;
    let operands: Vec<_> = ids.iter().map(String::as_str).collect();
    let command = paths.docker(&["container", "inspect", "--format", format], &operands);
    let output = process::capture_bounded(paths, &command, deadline, ids.len() * max_bytes)?;
    let text = std::str::from_utf8(&output).map_err(|_| invalid())?;
    if text.lines().count() != ids.len() {
        return Err(invalid());
    }
    text.lines()
        .zip(ids)
        .map(|(line, id)| {
            let mut seen: ProjectContainer = serde_json::from_str(line).map_err(|_| invalid())?;
            let name = seen
                .name
                .strip_prefix('/')
                .filter(|name| is_engine_name(name));
            if seen.id != *id || seen.project != project {
                return Err(invalid());
            }
            seen.name = name.ok_or_else(invalid)?.to_owned();
            Ok(seen)
        })
        .collect()
}

pub(in crate::control) fn service_hashes(
    paths: &Paths,
    project: &Project<'_>,
    deadline: Instant,
) -> Result<Option<BTreeMap<String, String>>, Fault> {
    let command = paths.compose(project, &["config", "--hash=*"]);
    let output = process::capture_bounded(paths, &command, deadline, 16 * 1024)?;
    let Ok(text) = std::str::from_utf8(&output) else {
        return Ok(None);
    };
    let mut hashes = BTreeMap::new();
    for line in text.lines() {
        let Some((service, hash)) = line.split_once(' ') else {
            return Ok(None);
        };
        if !compose_rules::forms::is_name(service)
            || !is_lower_hex(hash, 64)
            || hashes.insert(service.to_owned(), hash.to_owned()).is_some()
        {
            return Ok(None);
        }
    }
    Ok(Some(hashes))
}

const fn invalid() -> Fault {
    Fault::Process(io::ErrorKind::InvalidData)
}
