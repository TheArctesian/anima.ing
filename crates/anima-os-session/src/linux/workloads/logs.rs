//! Read a bounded, timestamped snapshot of the selected container's output.
use super::{
    DOCKER, Error, Result, command, docker, docker_args, valid_container, verify_system_container,
};
use serde::Deserialize;
use serde_json::{Value, json};

const DEFAULT_LINES: u16 = 200;
const MAX_LINES: u16 = 1000;
const TARGET: &str = concat!(
    r#"{"id":{{json .Id}},"#,
    r#""workspace":{{with index .Config "Labels"}}{{json (index . "ing.anima.workspace")}}{{else}}null{{end}}}"#,
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    container: String,
    lines: Option<u16>,
}

#[derive(Deserialize)]
struct Target {
    id: String,
    workspace: Option<String>,
}

pub(super) async fn read(args: Value) -> Result<Value> {
    let request: Request = serde_json::from_value(args)?;
    valid_container(&request.container)?;
    let lines = request.lines.unwrap_or(DEFAULT_LINES);
    if !(1..=MAX_LINES).contains(&lines) {
        return Err(Error::Refused("Choose between 1 and 1000 log lines."));
    }
    let observed = docker(
        &[
            "container",
            "inspect",
            "--format",
            TARGET,
            &request.container,
        ],
        15,
    )
    .await?;
    let target: Target = serde_json::from_str(&observed)?;
    verify_system_container(&request.container, &target.id, target.workspace.as_deref())?;
    let (stdout, stderr) = command::streams(
        DOCKER,
        &docker_args(&[
            "container",
            "logs",
            "--timestamps",
            "--tail",
            &lines.to_string(),
            &target.id,
        ]),
        15,
    )
    .await?;
    Ok(json!({"container":request.container,"stdout":stdout,"stderr":stderr,"tail":lines}))
}
