//! The details of one container, read-only (HOST-017).
//!
//! The format asks Docker for named values only. For the environment it asks for the text
//! before the first `=` of each entry, so no value of an environment variable leaves the
//! device (LOG-007). The output of a health check is not asked for.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::{
    Error,
    docker::{failure, says_no_container, state},
    model::{
        ContainerDetail, ContainerId, ContainerState, Health, HealthCheck, HealthRun, Label, Mount,
        NetworkAddress, Process,
    },
};
use crate::fleet::ssh::{CommandOutput, FixedCommand};

const DETAIL_FORMAT: &str = concat!(
    r#"'{"id":{{json .Id}},"name":{{json .Name}},"image":{{json .Config.Image}},"#,
    r#""imageId":{{json .Image}},"path":{{json .Path}},"args":{{json .Args}},"#,
    r#""workingDir":{{json .Config.WorkingDir}},"user":{{json .Config.User}},"#,
    r#""restart":{{json .HostConfig.RestartPolicy}},"state":{{json .State.Status}},"#,
    r#""exitCode":{{json .State.ExitCode}},"startedAt":{{json .State.StartedAt}},"#,
    r#""finishedAt":{{json .State.FinishedAt}},"restarts":{{json .RestartCount}},"#,
    r#""check":{{json .Config.Healthcheck}},"#,
    r#""health":{{with .State.Health}}{"status":{{json .Status}},"#,
    r#""failing":{{json .FailingStreak}},"runs":[{{range $i, $run := .Log}}{{if $i}},{{end}}"#,
    r#"{"end":{{json $run.End}},"exit":{{json $run.ExitCode}}}{{end}}]}{{else}}null{{end}},"#,
    r#""mounts":{{json .Mounts}},"networks":{{json .NetworkSettings.Networks}},"#,
    r#""labels":{{json .Config.Labels}},"#,
    r#""env":[{{range $i, $e := .Config.Env}}{{if $i}},{{end}}"#,
    r#"{{json (index (split $e "=") 0)}}{{end}}]}'"#,
);

pub(super) fn command(container: &ContainerId) -> FixedCommand {
    FixedCommand::new("docker")
        .arg("container")
        .arg("inspect")
        .arg("--format")
        .arg(DETAIL_FORMAT)
        .hex(container.digits())
        .arg("2>&1")
        .arg("&&")
        .arg("docker")
        .arg("top")
        .hex(container.digits())
        .arg("2>&1")
}

/// Docker writes `null` for an empty list or map. Only `id` and `name` must be there, so a value
/// that a Docker does not write does not hide the rest.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Inspected {
    id: String,
    name: String,
    #[serde(default)]
    image: String,
    #[serde(default)]
    image_id: String,
    #[serde(default)]
    path: String,
    args: Option<Vec<String>>,
    #[serde(default)]
    working_dir: String,
    #[serde(default)]
    user: String,
    restart: Option<Restart>,
    #[serde(default)]
    state: String,
    #[serde(default)]
    exit_code: i32,
    #[serde(default)]
    started_at: String,
    #[serde(default)]
    finished_at: String,
    #[serde(default)]
    restarts: u32,
    check: Option<Check>,
    health: Option<HealthRead>,
    mounts: Option<Vec<MountRead>>,
    networks: Option<BTreeMap<String, Option<Endpoint>>>,
    labels: Option<BTreeMap<String, String>>,
    env: Option<Vec<String>>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
struct Restart {
    name: String,
    maximum_retry_count: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Check {
    test: Option<Vec<String>>,
    /// Nanoseconds.
    interval: Option<u64>,
}

#[derive(Deserialize)]
struct HealthRead {
    #[serde(default)]
    status: String,
    #[serde(default)]
    failing: u32,
    #[serde(default)]
    runs: Vec<RunRead>,
}

#[derive(Deserialize)]
struct RunRead {
    #[serde(default)]
    end: String,
    #[serde(default)]
    exit: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase", default)]
struct MountRead {
    #[serde(rename = "Type")]
    kind: String,
    name: Option<String>,
    source: String,
    destination: String,
    #[serde(rename = "RW")]
    writable: bool,
}

impl Default for MountRead {
    fn default() -> Self {
        Self {
            kind: String::new(),
            name: None,
            source: String::new(),
            destination: String::new(),
            writable: true,
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Endpoint {
    #[serde(rename = "IPAddress")]
    ipv4: String,
    #[serde(rename = "GlobalIPv6Address")]
    ipv6: String,
    #[serde(rename = "Gateway")]
    gateway: String,
    #[serde(rename = "MacAddress")]
    mac: String,
}

const DETAIL_START: &str = r#"{"id":"#;

/// Both commands also write standard error to the output, so a warning of Docker can be before
/// either part.
pub(super) fn detail(output: &CommandOutput) -> Result<ContainerDetail, Error> {
    if output.cut {
        return Err(Error::Unreadable);
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();
    let Some(line) = lines.find(|line| line.starts_with(DETAIL_START)) else {
        return Err(match output.exit_status {
            Some(0) => Error::Unreadable,
            _ if says_no_container(&text) => Error::NoContainer,
            _ => failure(output),
        });
    };
    let read = serde_json::from_str::<Inspected>(line).map_err(|_| Error::Unreadable)?;
    Ok(container(read, processes(lines)))
}

fn container(read: Inspected, processes: Vec<Process>) -> ContainerDetail {
    let mut command = vec![read.path];
    command.extend(read.args.unwrap_or_default());
    let health = read.health.as_ref();
    let state = state(&read.state);
    ContainerDetail {
        id: read.id,
        name: read.name.trim_start_matches('/').to_owned(),
        image: read.image,
        image_id: read.image_id,
        command,
        working_dir: read.working_dir,
        user: read.user,
        restart_policy: restart_policy(&read.restart.unwrap_or_default()),
        state,
        exit_code: read.exit_code,
        started_at: instant(&read.started_at),
        finished_at: instant(&read.finished_at),
        restart_count: read.restarts,
        health_check: read.check.and_then(health_check),
        health: health
            .filter(|_| state == ContainerState::Running)
            .map_or(Health::None, |health| health_of(&health.status)),
        failing_streak: health.map_or(0, |health| health.failing),
        last_health_run: health
            .and_then(|health| health.runs.last())
            .map(|run| HealthRun {
                ended_at: instant(&run.end),
                exit_code: run.exit,
            }),
        mounts: read
            .mounts
            .unwrap_or_default()
            .into_iter()
            .map(mount)
            .collect(),
        networks: read
            .networks
            .unwrap_or_default()
            .into_iter()
            .map(|(name, endpoint)| address(name, endpoint.unwrap_or_default()))
            .collect(),
        labels: read
            .labels
            .unwrap_or_default()
            .into_iter()
            .map(|(name, value)| Label { name, value })
            .collect(),
        environment_names: read.env.unwrap_or_default(),
        processes,
    }
}

fn restart_policy(restart: &Restart) -> String {
    match (restart.name.as_str(), restart.maximum_retry_count) {
        ("", _) => "no".to_owned(),
        (name, 0) => name.to_owned(),
        (name, limit) => format!("{name}:{limit}"),
    }
}

fn health_check(check: Check) -> Option<HealthCheck> {
    let test = check.test.filter(|test| !test.is_empty())?;
    if test.first().map(String::as_str) == Some("NONE") {
        return None;
    }
    Some(HealthCheck {
        test,
        interval_seconds: check
            .interval
            .and_then(|nanoseconds| u32::try_from(nanoseconds / 1_000_000_000).ok()),
    })
}

fn health_of(status: &str) -> Health {
    match status {
        "healthy" => Health::Healthy,
        "unhealthy" => Health::Unhealthy,
        "starting" => Health::Starting,
        _ => Health::None,
    }
}

/// `None` for the year 1, which Docker writes for "never", and for text that is not a time.
fn instant(text: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|at| at.with_timezone(&Utc))
        .filter(|at| at.timestamp() > 0)
}

fn mount(read: MountRead) -> Mount {
    Mount {
        kind: read.kind,
        name: read.name.filter(|name| !name.is_empty()),
        source: read.source,
        destination: read.destination,
        read_only: !read.writable,
    }
}

fn address(network: String, endpoint: Endpoint) -> NetworkAddress {
    let filled = |text: String| Some(text).filter(|text| !text.is_empty());
    NetworkAddress {
        network,
        ipv4: filled(endpoint.ipv4),
        ipv6: filled(endpoint.ipv6),
        gateway: filled(endpoint.gateway),
        mac_address: filled(endpoint.mac),
    }
}

fn processes<'a>(mut lines: impl Iterator<Item = &'a str>) -> Vec<Process> {
    let header = lines.find(|line| line.split_whitespace().any(|column| column == "PID"));
    let columns: Vec<&str> = header.unwrap_or_default().split_whitespace().collect();
    let Some(pid) = columns.iter().position(|column| *column == "PID") else {
        return Vec::new();
    };
    let user = columns
        .iter()
        .position(|column| matches!(*column, "UID" | "USER"));
    lines
        .filter_map(|line| process(line, columns.len(), (pid, user)))
        .collect()
}

fn process(line: &str, columns: usize, (pid, user): (usize, Option<usize>)) -> Option<Process> {
    let mut rest = line.trim_start();
    let mut words = Vec::with_capacity(columns);
    for _ in 1..columns {
        let (word, after) = rest.split_once(char::is_whitespace)?;
        words.push(word);
        rest = after.trim_start();
    }
    Some(Process {
        pid: words.get(pid)?.parse().ok()?,
        user: user
            .and_then(|user| words.get(user))
            .map(|user| (*user).to_owned()),
        command: rest.to_owned(),
    })
}
