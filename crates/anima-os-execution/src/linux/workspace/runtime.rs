use super::{
    Error, Result, command,
    record::{self, Phase, Record},
};
use serde_json::{Value, json};

const ISOLATION: &[&str] = &[
    "--read-only",
    "--cap-drop=ALL",
    "--security-opt=no-new-privileges",
    "--init",
    "--user=1000:1000",
    "--restart=no",
    "--oom-score-adj=500",
    "--tmpfs",
    "/tmp:rw,nosuid,nodev,size=268435456",
    "--tmpfs",
    "/run:rw,nosuid,nodev,size=67108864",
    "--tmpfs",
    "/run/anima:rw,nosuid,nodev,noexec,size=1048576,uid=1000,gid=1000,mode=0755",
];

pub(super) struct Prepared {
    image: String,
    container: String,
}

impl Prepared {
    pub(super) async fn start(self) -> Result<String> {
        command::docker(&["start", &self.container]).await?;
        Ok(self.image)
    }
}

pub(super) async fn prepare(state: &Record) -> Result<Prepared> {
    let image = image_identity().await?;
    create_network(state).await?;
    let container = create_container(state, &image).await?;
    Ok(Prepared { image, container })
}

async fn image_identity() -> Result<String> {
    let image = std::fs::read_to_string("/etc/anima/workspace-image")?;
    let image = image.trim();
    if !image.starts_with("anima-workspace:")
        || image.len() > 128
        || image.contains(char::is_whitespace)
    {
        return Err(Error::Refused(
            "The packaged workspace image reference is invalid.",
        ));
    }
    let digest = command::docker(&["image", "inspect", "--format", "{{.Id}}", image]).await?;
    let digest = digest.trim();
    if digest.len() != 71
        || !digest.starts_with("sha256:")
        || !digest[7..].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(Error::Refused(
            "The workspace image is not available by immutable identity.",
        ));
    }
    Ok(digest.to_owned())
}

async fn create_network(state: &Record) -> Result<()> {
    let network = state.network();
    let bridge = format!("anaw{}", &state.workspace.simple().to_string()[..11]);
    let workspace_label = format!("ing.anima.workspace={}", state.workspace);
    command::docker(&[
        "network",
        "create",
        "--driver",
        "bridge",
        "--label",
        &workspace_label,
        "--opt",
        "com.docker.network.bridge.enable_icc=false",
        "--opt",
        &format!("com.docker.network.bridge.name={bridge}"),
        &network,
    ])
    .await?;
    Ok(())
}

async fn create_container(state: &Record, digest: &str) -> Result<String> {
    let network = state.network();
    let workspace_label = format!("ing.anima.workspace={}", state.workspace);
    let resources = state
        .resources
        .ok_or(Error::Refused("Workspace limits are missing."))?;
    let run = state
        .run
        .ok_or(Error::Refused("Workspace reservation is missing."))?;
    let home = record::home(state.workspace);
    let mount = format!("type=bind,source={},target=/home/agent", home.display());
    let memory = resources.memory_bytes.to_string();
    let cpus = format!(
        "{}.{:03}",
        resources.cpu_millis / 1000,
        resources.cpu_millis % 1000
    );
    let container = state.container();
    let run_label = format!("ing.anima.run={run}");
    let generation_label = format!("ing.anima.generation={}", state.generation);
    let tasks = resources.tasks.to_string();
    let mut arguments = vec![
        "create",
        "--name",
        &container,
        "--label",
        &workspace_label,
        "--label",
        &run_label,
        "--label",
        &generation_label,
        "--network",
        &network,
        "--dns=1.1.1.1",
        "--dns=1.0.0.1",
        "--cgroup-parent=anima-agents.slice",
        "--memory",
        &memory,
        "--memory-swap",
        &memory,
        "--cpus",
        &cpus,
        "--pids-limit",
        &tasks,
        "--mount",
        &mount,
        digest,
    ];
    arguments.splice(1..1, ISOLATION.iter().copied());
    let output = command::docker(&arguments).await?;
    let id = output.trim();
    if id.len() != 64 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::Refused(
            "Docker did not return a workspace container ID.",
        ));
    }
    Ok(id.to_owned())
}

pub async fn observe(state: &Record) -> Result<Value> {
    let value = inspect(state).await?;
    let ready = if value["running"] == true && state.checkout_ready() {
        runner_ready(&state.container()).await
    } else {
        false
    };
    Ok(json!({"workspace":state,"runtime":value,"runnerReady":ready}))
}

pub async fn inspect(state: &Record) -> Result<Value> {
    let text = command::docker(&["inspect", "--format",
        r#"{"running":{{json .State.Running}},"status":{{json .State.Status}},"exitCode":{{json .State.ExitCode}},"oomKilled":{{json .State.OOMKilled}},"run":{{json (index .Config.Labels "ing.anima.run")}},"generation":{{json (index .Config.Labels "ing.anima.generation")}}}"#,
        &state.container()]).await?;
    let value: Value = serde_json::from_str(&text)?;
    if value["run"].as_str() != state.run.map(|run| run.to_string()).as_deref()
        || value["generation"].as_str() != Some(state.generation.to_string().as_str())
    {
        return Err(Error::Refused(
            "The runtime identity does not match the workspace generation.",
        ));
    }
    Ok(value)
}

pub async fn stop(state: &Record) -> Result<()> {
    if matches!(state.phase, Phase::Prepared | Phase::Preparing) {
        return Err(Error::Refused("This workspace has no running generation."));
    }
    let present = command::docker(&[
        "ps",
        "--all",
        "--quiet",
        "--filter",
        &format!("name=^{}$", state.container()),
    ])
    .await?;
    if !present.trim().is_empty() {
        inspect(state).await?;
        command::docker(&["stop", "--time", "20", &state.container()]).await?;
        command::docker(&["rm", &state.container()]).await?;
    }
    let network = command::docker(&[
        "network",
        "ls",
        "--quiet",
        "--filter",
        &format!("name=^{}$", state.network()),
    ])
    .await?;
    if !network.trim().is_empty() {
        let owner = command::docker(&[
            "network",
            "inspect",
            "--format",
            "{{index .Labels \"ing.anima.workspace\"}}",
            &state.network(),
        ])
        .await?;
        if owner.trim() != state.workspace.to_string() {
            return Err(Error::Refused(
                "The network belongs to a different workspace.",
            ));
        }
        command::docker(&["network", "rm", &state.network()]).await?;
    }
    Ok(())
}

async fn runner_ready(container: &str) -> bool {
    let reply = command::docker_input(
        &[
            "exec",
            "--interactive",
            "--user=0:0",
            container,
            "/bin/socat",
            "-t",
            "5",
            "STDIO",
            "UNIX-CONNECT:/run/anima/owner.sock",
        ],
        b"{\"id\":0,\"call\":\"list\"}\n",
    )
    .await;
    reply.is_ok_and(|reply| {
        reply.lines().any(|line| {
            serde_json::from_str::<Value>(line).is_ok_and(|frame| {
                frame["frame"] == "done"
                    && frame["id"] == 0
                    && frame["result"]["outcome"] == "threads"
            })
        })
    })
}
