//! Root-owned reservations serialize admission with host maintenance.
use crate::{
    Admission, Error, RECEIPT,
    capacity::{self, Capacity, Policy, Resources},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};
use uuid::Uuid;

mod build;
pub use build::release_build;

const DIRECTORY: &str = "/persist/anima-admission/reservations";
const LIMIT: usize = 1024;

pub(super) fn count() -> Result<usize, Error> {
    let mut count = 0;
    for entry in fs::read_dir(DIRECTORY)?.take(LIMIT + 1) {
        entry?;
        count += 1;
    }
    if count > LIMIT {
        return Err(Error::Refused(
            "The agent and build reservation inventory exceeded its limit.",
        ));
    }
    Ok(count)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeOwner {
    #[default]
    Docker,
    SystemdBuild,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reservation {
    pub schema: u32,
    #[serde(default)]
    pub runtime: RuntimeOwner,
    pub run: Uuid,
    pub generation: u64,
    pub resources: Resources,
}

impl Reservation {
    fn require_pipeline(&self, pipeline: crate::source_pipeline::Pipeline) -> Result<(), Error> {
        let expected = match self.runtime {
            RuntimeOwner::Docker => pipeline.execution(),
            RuntimeOwner::SystemdBuild => pipeline.build(),
        };
        if self.run != expected || self.generation != 1 {
            return Err(Error::Refused(
                "The runtime reservation does not match source pipeline admission.",
            ));
        }
        Ok(())
    }
}

fn path(run: Uuid) -> PathBuf {
    Path::new(DIRECTORY).join(format!("{run}.json"))
}

fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, Error> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(Error::Refused("Admission state is not a regular file."));
    }
    let mut bytes = Vec::new();
    file.take(16385).read_to_end(&mut bytes)?;
    if bytes.len() > 16384 {
        return Err(Error::Refused("Admission state exceeded its limit."));
    }
    Ok(serde_json::from_slice(&bytes)?)
}

fn observed() -> Result<Capacity, Error> {
    let text = fs::read_to_string("/proc/meminfo")?;
    let bytes = |key: &str| -> Result<u64, Error> {
        let value = text
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse::<u64>().ok())
            .and_then(|value| value.checked_mul(1024));
        value.ok_or(Error::Refused("Available host memory is unknown."))
    };
    // Reservations cover sibling runtime scopes, not the controller's own CPU quota.
    let cpus = nix::unistd::sysconf(nix::unistd::SysconfVar::_NPROCESSORS_ONLN)
        .map_err(std::io::Error::from)?
        .and_then(|count| u64::try_from(count).ok())
        .filter(|count| *count > 0)
        .and_then(|count| count.checked_mul(1000))
        .ok_or(Error::Refused("Online node CPU capacity is unknown."))?;
    Ok(Capacity {
        total_memory: bytes("MemTotal:")?,
        available_memory: bytes("MemAvailable:")?,
        cpu_millis: cpus,
    })
}

fn policy() -> Result<Policy, Error> {
    let policy_source = fs::canonicalize("/etc/anima/admission.json")?;
    if !policy_source.starts_with("/nix/store") {
        return Err(Error::Refused(
            "The admission policy is not from the packaged system.",
        ));
    }
    read(&policy_source)
}

/// Observe remaining capacity under the same admission lock and policy as reservation.
/// # Errors
/// Unreconciled state, invalid reservations or unavailable capacity observations.
pub fn available(guard: &Admission) -> Result<Resources, Error> {
    crate::recovery::require_ready()?;
    if Path::new(RECEIPT).try_exists()? {
        return Err(Error::Refused("The node is in maintenance."));
    }
    guard.require_open_work()?;
    let mut used = Resources::default();
    for reservation in list(guard)? {
        used = used.add(reservation.resources)?;
    }
    capacity::available(&policy()?, observed()?, used)
}

/// Persists acceptance before a process can be started. A retry must carry the
/// same run, generation and limits. A retained reservation still needs open
/// admission before dispatch. The runtime owner must prove termination before
/// returning reserved capacity.
/// # Errors
/// Maintenance, conflicting retry, invalid accounting or insufficient free capacity.
pub fn reserve(guard: &Admission, request: Reservation) -> Result<Reservation, Error> {
    crate::recovery::require_ready()?;
    if Path::new(RECEIPT).try_exists()? {
        return Err(Error::Refused(
            "The node is in maintenance. Queue the work or choose another node.",
        ));
    }
    if request.schema != 1 || request.generation == 0 || request.run.is_nil() {
        return Err(Error::Refused("The work reservation identity is invalid."));
    }
    if let Some(pipeline) = guard.pipeline {
        request.require_pipeline(pipeline)?;
    }
    let existing = path(request.run).try_exists()?;
    if existing && read::<Reservation>(&path(request.run))? != request {
        return Err(Error::Refused(
            "The run already owns a different reservation.",
        ));
    }
    guard.require_open_work()?;
    let mut used = Resources::default();
    for (count, entry) in fs::read_dir(DIRECTORY)?.enumerate() {
        if count >= LIMIT {
            return Err(Error::Refused("Too many outstanding work reservations."));
        }
        let entry = entry?;
        let reservation: Reservation = read(&entry.path())?;
        if reservation.schema != 1 || entry.path() != path(reservation.run) {
            return Err(Error::Refused(
                "An existing reservation needs reconciliation.",
            ));
        }
        if reservation.run != request.run {
            used = used.add(reservation.resources)?;
        }
    }
    let policy = policy()?;
    capacity::admit(&policy, observed()?, used, request.resources)?;
    if existing {
        return Ok(request);
    }
    let pending = Path::new("/persist/anima-admission").join(format!("{}.pending", request.run));
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(&pending)?;
    file.write_all(&serde_json::to_vec(&request)?)?;
    file.sync_all()?;
    fs::hard_link(&pending, path(request.run))?;
    File::open(DIRECTORY)?.sync_all()?;
    fs::remove_file(pending)?;
    File::open("/persist/anima-admission")?.sync_all()?;
    Ok(request)
}

/// Release only after the runtime removed every container carrying the run's
/// identity. A missing or unhealthy engine is not proof of termination.
/// # Errors
/// Stale generation, active runtime ownership or unknown engine state.
pub async fn release_stopped(_guard: &Admission, run: Uuid, generation: u64) -> Result<(), Error> {
    use std::{process::Stdio, time::Duration};
    use tokio::{io::AsyncReadExt, process::Command};
    let present = path(run).try_exists()?;
    if present && {
        let reservation = read::<Reservation>(&path(run))?;
        reservation.generation != generation || reservation.runtime != RuntimeOwner::Docker
    } {
        return Err(Error::Refused("The reservation generation changed."));
    }
    let mut child = Command::new("/run/current-system/sw/bin/docker")
        .args([
            "--config",
            "/etc/anima/docker-client",
            "--host",
            "unix:///run/docker.sock",
            "ps",
            "--all",
            "--quiet",
            "--filter",
            &format!("label=ing.anima.run={run}"),
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or(Error::Refused("Runtime state is unavailable."))?;
    let observe = async {
        let mut bytes = Vec::new();
        stdout.take(65537).read_to_end(&mut bytes).await?;
        if bytes.len() > 65536
            || !child.wait().await?.success()
            || !bytes.iter().all(u8::is_ascii_whitespace)
        {
            return Err(Error::Refused(
                "The runtime has not proved termination. Capacity remains reserved.",
            ));
        }
        Ok(())
    };
    tokio::time::timeout(Duration::from_secs(10), observe)
        .await
        .map_err(|_| Error::Refused("Runtime reconciliation timed out."))??;
    if present {
        fs::remove_file(path(run))?;
    }
    File::open(DIRECTORY)?.sync_all()?;
    Ok(())
}

/// Checks existing ownership without admitting another process or changing capacity.
/// # Errors
/// Refuses missing, malformed or mismatched durable reservation state.
pub fn require(_guard: &Admission, expected: &Reservation) -> Result<(), Error> {
    if read::<Reservation>(&path(expected.run))? != *expected {
        return Err(Error::Refused(
            "The runtime reservation needs reconciliation.",
        ));
    }
    Ok(())
}

/// Enumerate durable ownership while admission is serialized.
/// # Errors
/// Refuses an oversized or malformed reservation inventory.
pub fn list(_guard: &Admission) -> Result<Vec<Reservation>, Error> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(DIRECTORY)? {
        if entries.len() >= 1024 {
            return Err(Error::Refused("Too many outstanding work reservations."));
        }
        let entry = entry?;
        let value: Reservation = read(&entry.path())?;
        if value.schema != 1
            || value.run.is_nil()
            || value.generation == 0
            || entry.path() != path(value.run)
        {
            return Err(Error::Refused("Reservation identity needs reconciliation."));
        }
        entries.push(value);
    }
    Ok(entries)
}
