//! Read only the Docker facts required for an exact-ID container action.
use std::{io, path::Path, time::Instant};

use serde::Deserialize;

use crate::{
    Action, Container, Installed, Inventory, Paths, Records,
    execution::{Fault, check_socket, files::Store, process},
    forms::{is_lower_hex, is_uuid},
};

const INSPECT: &str = concat!(
    r#"{"id":{{json .Id}},"name":{{json .Name}},"running":{{json .State.Running}},"#,
    r#""project":{{json (index .Config.Labels "com.docker.compose.project")}},"#,
    r#""restart":{{json .HostConfig.RestartPolicy.Name}}}"#,
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inspection {
    id: String,
    name: String,
    running: bool,
    project: Option<String>,
    restart: String,
}

pub(super) fn store(paths: &Paths, action: Action) -> Result<Store<'_>, Fault> {
    let root = Path::new(&paths.root);
    if action == Action::Stop {
        Store::observe(root)
    } else {
        Store::open(root)
    }
}

pub(super) fn observe(
    store: &Store<'_>,
    paths: &Paths,
    (id, action): (&str, Action),
    deadline: Instant,
) -> Result<(Inventory, Records), Fault> {
    let container = inspected(paths, id, deadline)?;
    let application = container
        .as_ref()
        .and_then(|one| one.project.as_deref())
        .and_then(application);
    let installed = if let Some(application) = application {
        let folders = paths.application(&application);
        if action != Action::Stop {
            crate::migration::check_start(store, (&folders, &paths.socket), deadline)?;
        }
        store
            .has_private_file(&folders.final_form())?
            .then(|| installed(application))
    } else {
        None
    };
    Ok((
        Inventory {
            containers: container.into_iter().collect(),
            ..Inventory::default()
        },
        Records {
            applications: installed.into_iter().collect(),
        },
    ))
}

fn inspected(paths: &Paths, id: &str, deadline: Instant) -> Result<Option<Container>, Fault> {
    if !is_lower_hex(id, 64) {
        return Err(Fault::UnsafePlan);
    }
    let filter = format!("id={id}");
    let find = paths.docker(
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
    let found = process::capture(paths, &find, deadline)?;
    let listed = std::str::from_utf8(&found).map_err(|_| invalid())?.trim();
    if listed.is_empty() {
        return Ok(None);
    }
    if listed != id {
        return Err(invalid());
    }
    let inspect = paths.docker(&["container", "inspect", "--format", INSPECT], &[id]);
    check_socket(paths)?;
    let bytes = process::capture(paths, &inspect, deadline)?;
    let data: Inspection = serde_json::from_slice(&bytes).map_err(|_| invalid())?;
    if data.id != id {
        return Err(invalid());
    }
    let name = data.name.strip_prefix('/').ok_or_else(invalid)?;
    Ok(Some(Container {
        id: data.id,
        name: name.to_owned(),
        project: data.project.filter(|project| !project.is_empty()),
        folder: None,
        running: data.running,
        restarts: matches!(
            data.restart.as_str(),
            "always" | "unless-stopped" | "on-failure"
        ),
    }))
}

pub(super) fn application(project: &str) -> Option<String> {
    let id = project
        .strip_prefix("anima-")
        .filter(|id| is_lower_hex(id, 32))?;
    let application = format!(
        "{}-{}-{}-{}-{}",
        &id[..8],
        &id[8..12],
        &id[12..16],
        &id[16..20],
        &id[20..]
    );
    is_uuid(&application).then_some(application)
}

const fn installed(application: String) -> Installed {
    Installed {
        application,
        has_files: true,
        current: Vec::new(),
        kept: None,
        pulled: Vec::new(),
    }
}

const fn invalid() -> Fault {
    Fault::Process(io::ErrorKind::InvalidData)
}
