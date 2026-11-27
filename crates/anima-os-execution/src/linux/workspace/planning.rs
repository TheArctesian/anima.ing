//! Planning inspects the managed workspace; it neither builds nor loads an image.
use super::{Error, Result, command, record, runtime};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{DirBuilderExt, OpenOptionsExt},
    path::Path,
};
use uuid::Uuid;

const ROOT: &str = "/persist/anima-workspaces/plans";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    schema: u32,
    id: Uuid,
    workspace: Uuid,
    generation: u64,
    phase: String,
    result: Option<Value>,
}

pub async fn prepare(workspace: Uuid, generation: u64, id: Uuid) -> Result<Value> {
    if id.is_nil() {
        return Err(Error::Refused("Choose a valid preparation identity."));
    }
    let admission = anima_os_maintenance::Admission::acquire()?;
    let _workspace = record::lock(workspace)?;
    let state = record::required(workspace)?;
    if state.generation != generation || state.phase != record::Phase::Running {
        return Err(Error::Refused("The workspace generation changed."));
    }
    if runtime::observe(&state).await?["runtime"]["running"] != true {
        return Err(Error::Refused("The workspace is not running."));
    }
    if !Path::new(ROOT).exists() {
        fs::DirBuilder::new().mode(0o700).create(ROOT)?;
    }
    let path = Path::new(ROOT).join(format!("{id}.json"));
    if path.try_exists()? {
        let previous = previous(&path, workspace, generation, id)?;
        return Ok(serde_json::to_value(previous)?);
    }
    let mut plan = Plan {
        schema: 1,
        id,
        workspace,
        generation,
        phase: "preparing".into(),
        result: None,
    };
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&path)?;
    file.write_all(&serde_json::to_vec(&plan)?)?;
    file.sync_all()?;
    fs::File::open(ROOT)?.sync_all()?;
    // Existing workspace limits include these processes. Release the global
    // admission lock before executing bounded work; no second runtime is launched.
    drop(admission);
    let outcome = execute(&state.container(), id).await;
    match outcome {
        Ok(value) => {
            plan.phase = "prepared".into();
            plan.result = Some(value);
        }
        Err(_) => {
            plan.phase = "interrupted".into();
        }
    }
    save(&path, &plan)?;
    Ok(serde_json::to_value(plan)?)
}

async fn execute(container: &str, id: Uuid) -> Result<Value> {
    if uses_dockerfile(container).await? {
        return Ok(json!({"strategy":"dockerfile","source":"live-workspace",
            "buildReady":false,"imageProduced":false,
            "next":"Capture and authorize an immutable source context before a build attempt."}));
    }
    let directory = format!("/tmp/anima-plan-{id}");
    command::docker(&[
        "exec",
        "--user=1000:1000",
        container,
        "/bin/mkdir",
        "-m",
        "700",
        &directory,
    ])
    .await?;
    let plan = format!("{directory}/railpack-plan.json");
    let info = format!("{directory}/railpack-info.json");
    command::docker(&[
        "exec",
        "--user=1000:1000",
        "--workdir=/home/agent/workspace",
        container,
        "/bin/timeout",
        "--signal=TERM",
        "--kill-after=5s",
        "45s",
        "/bin/railpack",
        "prepare",
        ".",
        "--plan-out",
        &plan,
        "--info-out",
        &info,
    ])
    .await?;
    let plan: Value = serde_json::from_str(
        &command::docker(&["exec", "--user=1000:1000", container, "/bin/cat", &plan]).await?,
    )?;
    let info: Value = serde_json::from_str(
        &command::docker(&["exec", "--user=1000:1000", container, "/bin/cat", &info]).await?,
    )?;
    if !plan.is_object() || !info.is_object() {
        return Err(Error::Refused(
            "The planner returned invalid preparation data.",
        ));
    }
    let result = json!({"strategy":"railpack","plan":plan,"info":info,
        "source":"live-workspace","buildReady":false,"imageProduced":false,
        "next":"Capture and authorize an immutable source context before a build attempt."});
    if serde_json::to_vec(&result)?.len() > 1_000_000 {
        return Err(Error::Refused(
            "The combined preparation exceeded its limit.",
        ));
    }
    Ok(result)
}

fn previous(path: &Path, workspace: Uuid, generation: u64, id: Uuid) -> Result<Plan> {
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)?;
    let mut bytes = Vec::new();
    file.take(1_048_577).read_to_end(&mut bytes)?;
    if bytes.len() > 1_048_576 {
        return Err(Error::Refused("Preparation state exceeded its limit."));
    }
    let previous: Plan = serde_json::from_slice(&bytes)?;
    if previous.schema != 1
        || previous.workspace != workspace
        || previous.generation != generation
        || previous.id != id
    {
        return Err(Error::Refused(
            "Preparation identity already belongs to another request.",
        ));
    }
    Ok(previous)
}

fn save(path: &Path, plan: &Plan) -> Result<()> {
    let pending = path.with_extension("pending");
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&pending)?;
    output.write_all(&serde_json::to_vec(plan)?)?;
    output.sync_all()?;
    fs::rename(pending, path)?;
    fs::File::open(ROOT)?.sync_all()?;
    Ok(())
}

async fn uses_dockerfile(container: &str) -> Result<bool> {
    let strategy = command::docker(&[
        "exec",
        "--user=1000:1000",
        "--workdir=/home/agent/workspace",
        container,
        "/bin/bash",
        "-c",
        "if [ -f Dockerfile ]; then printf dockerfile; else printf railpack; fi",
    ])
    .await?;
    match strategy.as_str() {
        "dockerfile" => Ok(true),
        "railpack" => Ok(false),
        _ => Err(Error::Refused(
            "The source strategy could not be determined.",
        )),
    }
}
