//! The two `docker` commands and the pure reading of their output (HOST-001, CHK-012).
//!
//! The list asks for no label but four of Compose (R-211), so a label that holds a secret never
//! leaves the device. The log asks for both streams (`2>&1`): the link never reads standard error.

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{
    Error,
    model::{Container, ContainerLog, ContainerState, Health, LogLine, Tail},
    project,
};
use crate::fleet::ssh::{CommandOutput, FixedCommand};

const LIST_FORMAT: &str = concat!(
    r#"'{"id":{{json .ID}},"names":{{json .Names}},"image":{{json .Image}},"#,
    r#""state":{{json .State}},"status":{{json .Status}},"ports":{{json .Ports}},"#,
    r#""created":{{json .CreatedAt}},"#,
    r#""project":{{json (.Label "com.docker.compose.project")}},"#,
    r#""service":{{json (.Label "com.docker.compose.service")}},"#,
    r#""dir":{{json (.Label "com.docker.compose.project.working_dir")}},"#,
    r#""files":{{json (.Label "com.docker.compose.project.config_files")}}}'"#,
);

/// The exit status of a shell that found no such program.
const NOT_FOUND_STATUS: u32 = 127;
const NO_SUCH_CONTAINER: [&str; 2] = [
    "Error response from daemon: No such container",
    "Error: No such container",
];

pub(super) fn list_command() -> FixedCommand {
    FixedCommand::new("docker")
        .arg("ps")
        .arg("-a")
        .arg("--no-trunc")
        .arg("--format")
        .arg(LIST_FORMAT)
}

pub(super) fn log_command(tail: &Tail) -> FixedCommand {
    FixedCommand::new("docker")
        .arg("logs")
        .arg("--timestamps")
        .arg("--tail")
        .number(tail.lines.get())
        .hex(tail.container.digits())
        .arg("2>&1")
}

#[derive(Deserialize)]
struct Line {
    id: String,
    names: String,
    image: String,
    state: String,
    status: String,
    ports: String,
    created: String,
    project: String,
    service: String,
    /// An older answer has no labels of where Compose found the project.
    #[serde(default)]
    dir: String,
    #[serde(default)]
    files: String,
}

pub(super) const fn failure(output: &CommandOutput) -> Error {
    match output.exit_status {
        Some(NOT_FOUND_STATUS) => Error::NoDocker,
        _ => Error::DockerFailed,
    }
}

/// # Errors
/// `Unreadable` for cut or unexpected output; `NoDocker` or `DockerFailed` for a failed command.
pub(super) fn containers(output: &CommandOutput) -> Result<Vec<Container>, Error> {
    if output.cut {
        return Err(Error::Unreadable);
    }
    if output.exit_status != Some(0) {
        return Err(failure(output));
    }
    let text = std::str::from_utf8(&output.stdout).map_err(|_| Error::Unreadable)?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<Line>(line)
                .map(container)
                .map_err(|_| Error::Unreadable)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|mut containers| {
            project::mark(&mut containers);
            containers
        })
}

fn container(line: Line) -> Container {
    Container {
        actions_available: false,
        managed_application_id: None,
        id: line.id,
        name: shown_name(&line.names),
        image: line.image,
        state: state(&line.state),
        health: health(&line.status),
        status: line.status,
        ports: listed(&line.ports),
        created_at: created(&line.created),
        compose_project: Some(line.project).filter(|project| !project.is_empty()),
        compose_service: Some(line.service).filter(|service| !service.is_empty()),
        compose_working_dir: Some(line.dir).filter(|dir| !dir.is_empty()),
        compose_files: line
            .files
            .split(',')
            .filter(|file| !file.is_empty())
            .map(str::to_owned)
            .collect(),
        project_state: None,
    }
}

/// The first name with no `/`: a link of another container gives a name with one.
pub(super) fn shown_name(names: &str) -> String {
    names
        .split(',')
        .find(|name| !name.contains('/'))
        .or_else(|| names.split(',').next())
        .unwrap_or_default()
        .to_owned()
}

pub(super) fn state(text: &str) -> ContainerState {
    match text {
        "running" => ContainerState::Running,
        "exited" => ContainerState::Exited,
        "restarting" => ContainerState::Restarting,
        "paused" => ContainerState::Paused,
        "created" => ContainerState::Created,
        "removing" => ContainerState::Removing,
        "dead" => ContainerState::Dead,
        _ => ContainerState::Unknown,
    }
}

fn health(status: &str) -> Health {
    if status.contains("(unhealthy)") {
        Health::Unhealthy
    } else if status.contains("(health: starting)") {
        Health::Starting
    } else if status.contains("(healthy)") {
        Health::Healthy
    } else {
        Health::None
    }
}

fn listed(ports: &str) -> Vec<String> {
    ports
        .split(", ")
        .filter(|port| !port.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Docker's `2026-11-18 10:00:00 -0800 PST`.
fn created(text: &str) -> Option<DateTime<Utc>> {
    let mut words = text.split_whitespace();
    let stamp = format!("{} {} {}", words.next()?, words.next()?, words.next()?);
    DateTime::parse_from_str(&stamp, "%Y-%m-%d %H:%M:%S %z")
        .ok()
        .map(|at| at.with_timezone(&Utc))
}

/// When the output was cut at the link's cap, the last line is partial and is left out.
///
/// # Errors
/// `NoContainer` when Docker knows no such container; else `NoDocker` or `DockerFailed`.
pub(super) fn log(output: &CommandOutput) -> Result<ContainerLog, Error> {
    let text = String::from_utf8_lossy(&output.stdout);
    if !output.cut && output.exit_status != Some(0) {
        return Err(if says_no_container(&text) {
            Error::NoContainer
        } else {
            failure(output)
        });
    }
    let mut lines: Vec<LogLine> = text.lines().map(log_line).collect();
    if output.cut {
        lines.pop();
    }
    Ok(ContainerLog {
        lines,
        cut: output.cut,
    })
}

/// Only the last line is read, from its start, so a line that the container wrote cannot forge it.
pub(super) fn says_no_container(text: &str) -> bool {
    text.lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .is_some_and(|line| {
            NO_SUCH_CONTAINER
                .iter()
                .any(|start| line.starts_with(start))
        })
}

fn log_line(line: &str) -> LogLine {
    let timed = line.split_once(' ').and_then(|(stamp, text)| {
        let at = DateTime::parse_from_rfc3339(stamp).ok()?;
        Some((at.with_timezone(&Utc), text))
    });
    match timed {
        Some((at, text)) => LogLine {
            at: Some(at),
            text: text.to_owned(),
        },
        None => LogLine {
            at: None,
            text: line.to_owned(),
        },
    }
}
