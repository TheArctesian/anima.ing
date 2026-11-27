//! Read the exact Podman source in its original account context.
mod capture;
mod launchers;
pub(super) use capture::{Snapshot, capture};

use std::{
    collections::BTreeSet,
    env,
    os::unix::process::CommandExt as _,
    path::PathBuf,
    process::{Command, Stdio},
    time::Instant,
};

use nix::unistd::{Uid, User};
use serde::Deserialize;

use crate::{execution::process, forms::is_path, startup::protected_program};

use super::{Error, Manifest};

const STORAGE: &str =
    r#"{"root":{{json .Store.GraphRoot}},"rootless":{{json .Host.Security.Rootless}}}"#;
const CONTAINER: &str = concat!(
    r#"{"id":{{json .ID}},"state":{{json .State.Status}},"pid":{{json .State.Pid}},"#,
    r#""running":{{json .State.Running}},"paused":{{json .State.Paused}},"#,
    r#""restarting":{{json .State.Restarting}},"restart":{{json .HostConfig.RestartPolicy.Name}},"#,
    r#""systemdUnit":{{json (index .Config.Labels "PODMAN_SYSTEMD_UNIT")}}}"#,
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Storage {
    root: String,
    rootless: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Container {
    id: String,
    state: String,
    pid: u64,
    running: bool,
    paused: bool,
    restarting: bool,
    restart: String,
    #[serde(rename = "systemdUnit")]
    systemd_unit: Option<String>,
}

pub(super) fn stopped(manifest: &Manifest, deadline: Instant) -> Result<String, Error> {
    let user = account(manifest.podman_uid)?;
    if let Some(source) = manifest.source() {
        return source.stopped(&user, deadline);
    }
    let units = stopped_containers(&user, &manifest.podman_containers, deadline)?;
    launchers::require_fenced(&user, &units, deadline)?;
    graph_root(&user, deadline)
}

fn account(uid: u32) -> Result<User, Error> {
    User::from_uid(Uid::from_raw(uid))
        .ok()
        .flatten()
        .filter(|user| user.dir.is_absolute() && !user.name.starts_with('-'))
        .ok_or(Error::SourceActive)
}

fn graph_root(user: &User, deadline: Instant) -> Result<String, Error> {
    let bytes = podman(user, &["info", "--format", STORAGE], deadline)?;
    let runtime: Storage = serde_json::from_slice(&bytes).map_err(|_| Error::SourceActive)?;
    if runtime.rootless == user.uid.is_root() || !is_path(&runtime.root) {
        return Err(Error::SourceActive);
    }
    Ok(runtime.root)
}

fn stopped_containers(
    user: &User,
    expected: &[String],
    deadline: Instant,
) -> Result<BTreeSet<String>, Error> {
    let mut arguments = vec!["container", "inspect", "--format", CONTAINER, "--"];
    arguments.extend(expected.iter().map(String::as_str));
    let storage = podman(user, &arguments, deadline)?;
    let records = std::str::from_utf8(&storage).map_err(|_| Error::SourceActive)?;
    let mut ids = Vec::new();
    let mut units = BTreeSet::new();
    for line in records.lines() {
        let row: Container = serde_json::from_str(line).map_err(|_| Error::SourceActive)?;
        if !matches!(
            row.state.as_str(),
            "exited" | "stopped" | "configured" | "created"
        ) || row.pid != 0
            || row.running
            || row.paused
            || row.restarting
            || !matches!(row.restart.as_str(), "" | "no" | "never")
        {
            return Err(Error::SourceActive);
        }
        ids.push(row.id);
        units.extend(row.systemd_unit.filter(|name| !name.is_empty()));
    }
    ids.sort_unstable();
    if ids != expected {
        return Err(Error::SourceActive);
    }
    Ok(units)
}

fn podman(user: &User, arguments: &[&str], deadline: Instant) -> Result<Vec<u8>, Error> {
    let mut local = vec!["--remote=false"];
    local.extend_from_slice(arguments);
    query(user, "podman", &local, deadline)
}

fn query(user: &User, tool: &str, args: &[&str], deadline: Instant) -> Result<Vec<u8>, Error> {
    let tools = env::var_os("ANIMA_LEGACY_BIN").map_or_else(
        || PathBuf::from("/run/current-system/sw/bin"),
        PathBuf::from,
    );
    let binary = protected_program(&tools.join(tool)).map_err(|_| Error::SourceActive)?;
    let mut command = if user.uid.is_root() {
        Command::new(&binary)
    } else {
        let runuser = protected_program(&tools.join("runuser")).map_err(|_| Error::SourceActive)?;
        let mut run = Command::new(runuser);
        run.args(["-u", &user.name, "--"]).arg(&binary);
        run
    };
    command
        .env_clear()
        .env("HOME", &user.dir)
        .env("PATH", tools)
        .env("LC_ALL", "C")
        .current_dir("/")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .args(args);
    if !user.uid.is_root() {
        command.env("XDG_RUNTIME_DIR", format!("/run/user/{}", user.uid));
    }
    process::capture_program(command, deadline, 128 * 1024).map_err(|_| Error::SourceActive)
}
