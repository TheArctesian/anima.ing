use crate::{
    Error, drain, hosting, reconcile, reservations, source_capture, source_pipeline, update_window,
};
use serde::Serialize;
use std::{
    fs, io,
    process::Stdio,
    time::{Duration, SystemTime},
};
use tokio::{io::AsyncReadExt, process::Command};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Impact {
    drain: Option<drain::Drain>,
    update_window_ends_at: Option<u64>,
    reservations: Option<usize>,
    source_in_progress: Option<bool>,
    running_docker: Option<usize>,
    legacy_running: Option<bool>,
    hosting_clear: bool,
    pending_transition: bool,
    ready: bool,
}

/// Observe maintenance blockers without holding admission across the engine query.
///
/// # Errors
/// The current drain or power-transition record could not be read.
pub async fn impact() -> Result<Impact, Error> {
    let running_docker = docker_running().await.ok();
    let legacy_running = podman_running().ok();
    let reservations = reservations::count().ok();
    let source_in_progress = source_work().ok();
    let hosting_clear = hosting::available().is_ok();
    let pending_transition = !reconcile::status()?.is_null();
    Ok(Impact {
        drain: drain::status()?,
        update_window_ends_at: update_window::active_until(SystemTime::now())?,
        reservations,
        source_in_progress,
        running_docker,
        legacy_running,
        hosting_clear,
        pending_transition,
        ready: reservations == Some(0)
            && source_in_progress == Some(false)
            && running_docker == Some(0)
            && legacy_running == Some(false)
            && hosting_clear
            && !pending_transition,
    })
}

fn source_work() -> Result<bool, Error> {
    Ok(source_pipeline::active()?.is_some() || source_capture::active()?)
}

pub(super) async fn quiescent() -> Result<(), Error> {
    if reservations::count()? != 0 {
        return Err(Error::Refused(
            "Agent or build reservations are active. Drain them before maintenance.",
        ));
    }
    if source_pipeline::active()?.is_some() {
        return Err(Error::Refused(
            "An accepted source build is still running or reconciling.",
        ));
    }
    if source_capture::active()? {
        return Err(Error::Refused(
            "A source capture is still running. Wait for it before maintenance.",
        ));
    }
    if docker_running().await? != 0 {
        return Err(Error::Refused(
            "Docker containers are running. Stop them through their owners before maintenance.",
        ));
    }
    if podman_running()? {
        return Err(Error::Refused(
            "Legacy Podman workloads are running. Migrate or stop them before maintenance.",
        ));
    }
    Ok(())
}

async fn docker_running() -> Result<usize, Error> {
    let mut child = Command::new("/run/current-system/sw/bin/docker")
        .args([
            "--config",
            "/etc/anima/docker-client",
            "--host",
            &format!("unix://{}", hosting::SYSTEM_SOCKET),
            "ps",
            "--quiet",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or(Error::Refused("Docker inventory is unavailable."))?;
    let read = async {
        let mut bytes = Vec::new();
        stdout.take(65537).read_to_end(&mut bytes).await?;
        if bytes.len() > 65536 || !child.wait().await?.success() {
            return Err(Error::Refused(
                "Docker workload state is unknown. Maintenance was deferred.",
            ));
        }
        Ok(bytes
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.iter().all(u8::is_ascii_whitespace))
            .count())
    };
    tokio::time::timeout(Duration::from_secs(10), read)
        .await
        .map_err(|_| Error::Refused("Docker inventory timed out. Maintenance was deferred."))?
}

fn podman_running() -> Result<bool, Error> {
    // Rootless Podman workloads are not visible to the system Docker engine.
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        if !entry
            .file_name()
            .as_encoded_bytes()
            .iter()
            .all(u8::is_ascii_digit)
        {
            continue;
        }
        match fs::read_to_string(entry.path().join("comm")) {
            Ok(name) if name.trim() == "conmon" => return Ok(true),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(false)
}
