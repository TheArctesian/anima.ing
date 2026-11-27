//! Project the parts of Docker container inspection used by the local console.
mod resources;

use super::{ContainerRequest, Error, Result, docker, valid_container, verify_system_container};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const FORMAT: &str = concat!(
    r#"{"container":{{json .Id}},"name":{{json .Name}},"state":{{json .State.Status}},"#,
    r#""workspace":{{with index .Config "Labels"}}{{json (index . "ing.anima.workspace")}}{{else}}null{{end}},"#,
    r#""health":{{with index .State "Health"}}{{json .Status}}{{else}}null{{end}},"#,
    r#""limits":{{with index . "HostConfig"}}{"nanoCpus":{{json (index . "NanoCpus")}},"cpuQuota":{{json (index . "CpuQuota")}},"#,
    r#""cpuPeriod":{{json (index . "CpuPeriod")}},"cpuSet":{{json (index . "CpusetCpus")}},"#,
    r#""memoryBytes":{{json (index . "Memory")}},"tasks":{{json (index . "PidsLimit")}}}{{else}}null{{end}},"#,
    r#""networks":[{{$sep := ""}}{{with index . "NetworkSettings"}}{{range $name, $network := index . "Networks"}}{{$sep}}{{json $name}}{{$sep = ","}}{{end}}{{end}}],"#,
    r#""mounts":[{{range $i, $mount := index . "Mounts"}}{{if $i}},{{end}}"#,
    r#"{"kind":{{json $mount.Type}},"name":{{with index $mount "Name"}}{{json .}}{{else}}""{{end}},"#,
    r#""destination":{{json $mount.Destination}},"writable":{{json $mount.RW}}}{{end}}]}"#,
);

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Details {
    container: String,
    name: String,
    state: String,
    #[serde(skip_serializing)]
    workspace: Option<String>,
    #[serde(skip_serializing)]
    limits: Option<resources::Configuration>,
    health: Option<String>,
    networks: Vec<String>,
    mounts: Vec<Mount>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Mount {
    kind: String,
    name: String,
    destination: String,
    writable: bool,
}

pub(super) async fn read(args: Value) -> Result<Value> {
    let request: ContainerRequest = serde_json::from_value(args)?;
    valid_container(&request.container)?;
    let output = docker(
        &[
            "container",
            "inspect",
            "--format",
            FORMAT,
            &request.container,
        ],
        15,
    )
    .await
    .map_err(|_| {
        Error::Refused("Container details are unavailable. Refresh Workloads and try again.")
    })?;
    let mut details: Details = serde_json::from_str(&output)?;
    verify_system_container(
        &request.container,
        &details.container,
        details.workspace.as_deref(),
    )?;
    if details.name.starts_with('/') {
        details.name.remove(0);
    }
    let resources =
        resources::observe(&details.container, &details.state, details.limits.as_ref()).await;
    let mut reply = serde_json::to_value(details)?;
    reply["resources"] = resources;
    Ok(reply)
}
