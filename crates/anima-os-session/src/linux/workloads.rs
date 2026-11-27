//! Read-only inventory of the node's system Docker engine.
mod applications;
mod build_logs;
mod details;
mod hosting;
mod logs;

use super::{Error, Result, command};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Value, json};

const FORMAT: &str = concat!(
    r#"{"id":{{json .ID}},"name":{{json .Names}},"image":{{json .Image}},"#,
    r#""state":{{json .State}},"status":{{json .Status}},"health":{{json .HealthStatus}},"ports":{{json .Ports}},"#,
    r#""project":{{json (.Label "com.docker.compose.project")}},"service":{{json (.Label "com.docker.compose.service")}},"#,
    r#""folder":{{json (.Label "com.docker.compose.project.working_dir")}},"#,
    r#""finalForm":{{json (.Label "com.docker.compose.project.config_files")}},"#,
    r#""workspace":{{json (.Label "ing.anima.workspace")}}}"#,
);

#[derive(Deserialize, Serialize)]
struct Container {
    id: String,
    name: String,
    image: String,
    state: String,
    status: String,
    #[serde(default, deserialize_with = "optional_health")]
    health: Option<Health>,
    ports: String,
    project: String,
    service: String,
    #[serde(skip_serializing)]
    folder: String,
    #[serde(rename = "finalForm", skip_serializing)]
    final_form: String,
    #[serde(skip_serializing)]
    workspace: String,
    #[serde(skip_deserializing)]
    application: Option<String>,
    #[serde(rename = "applicationRecordUnavailable", skip_deserializing)]
    application_record_unavailable: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum Health {
    Starting,
    Healthy,
    Unhealthy,
}

fn optional_health<'de, D: Deserializer<'de>>(
    input: D,
) -> std::result::Result<Option<Health>, D::Error> {
    Ok(match Option::<String>::deserialize(input)?.as_deref() {
        Some("starting") => Some(Health::Starting),
        Some("healthy") => Some(Health::Healthy),
        Some("unhealthy") => Some(Health::Unhealthy),
        _ => None,
    })
}

impl Container {
    fn application_candidate(&self) -> bool {
        self.project.starts_with("anima-") && !self.service.is_empty()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContainerRequest {
    container: String,
}

pub async fn dispatch(operation: &str, args: Value) -> Result<Value> {
    match operation {
        "workloads" => list().await,
        "work-admission" if args.as_object().is_some_and(serde_json::Map::is_empty) => {
            Ok(work_admission())
        }
        "workload-details" => details::read(args).await,
        "workload-logs" => logs::read(args).await,
        "workload-build-logs" => build_logs::read(args).await,
        "workload-terminal-target" => terminal_target(args).await,
        _ => Err(Error::Refused("Unsupported workload operation.")),
    }
}

const EXECUTION: &str = "/run/current-system/sw/bin/anima-os-execution";
const DOCKER: &str = "/run/current-system/sw/bin/docker";

fn docker_args<'a>(arguments: &[&'a str]) -> Vec<&'a str> {
    let mut fixed = vec![
        "--config",
        "/etc/anima/docker-client",
        "--host",
        "unix:///run/docker.sock",
    ];
    fixed.extend_from_slice(arguments);
    fixed
}

async fn docker(arguments: &[&str], seconds: u64) -> Result<String> {
    command::run(DOCKER, &docker_args(arguments), None, seconds).await
}

fn work_admission() -> Value {
    json!(anima_os_maintenance::work_admission().ok())
}

async fn list() -> Result<Value> {
    let (docker_result, managed, hosting) = tokio::join!(
        docker(&["ps", "--all", "--no-trunc", "--format", FORMAT], 15),
        managed(true),
        hosting::observe(),
    );
    let docker_available = docker_result.is_ok();
    let output = docker_result.unwrap_or_default();
    let mut containers: Vec<_> = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(serde_json::from_str::<Container>)
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|container| container.workspace.is_empty())
        .collect();
    applications::mark(&mut containers).await;
    Ok(
        json!({"runtime":"docker","scope":"system","containers":containers,
        "dockerAvailable":docker_available,"managed":managed,"hosting":hosting,"workAdmission":work_admission()}),
    )
}

pub async fn managed(usage: bool) -> Option<Value> {
    let input = if usage {
        b"{\"operation\":\"inventory\",\"usage\":true}\n".as_slice()
    } else {
        b"{\"operation\":\"inventory\"}\n".as_slice()
    };
    let output = command::run(EXECUTION, &[], Some(input), 15).await.ok()?;
    let mut reply: Value = serde_json::from_str(&output).ok()?;
    (reply["ok"] == true).then(|| reply["value"].take())
}

async fn terminal_target(args: Value) -> Result<Value> {
    let request: ContainerRequest = serde_json::from_value(args)?;
    valid_container(&request.container)?;
    let response = command::run(
        "/run/current-system/sw/bin/anima-application-terminal",
        &["target", &request.container],
        None,
        20,
    )
    .await
    .map_err(|_| Error::Refused("This container is unavailable for local administration."))?;
    Ok(serde_json::from_str(&response)?)
}

fn verify_system_container(expected: &str, observed: &str, workspace: Option<&str>) -> Result<()> {
    if !observed.eq_ignore_ascii_case(expected) {
        return Err(Error::Refused(
            "Container identity changed. Refresh Workloads and try again.",
        ));
    }
    if workspace.is_some_and(|workspace| !workspace.is_empty()) {
        return Err(Error::Refused("Open this workspace in Anima's Code panel."));
    }
    Ok(())
}

fn valid_container(id: &str) -> Result<()> {
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Refused(
            "Choose a container from the current inventory.",
        ));
    }
    Ok(())
}
