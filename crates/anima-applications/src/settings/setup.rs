//! Own the initial system-Docker setup at the local administrator boundary.
use std::{
    io,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::json;

use super::{Settings, SettingsFault, read};
use crate::execution::{Fault, files::Store};

const SETTINGS: &str = "settings.json";
const PAIR: &str = "control/hub-pair.json";

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SetupObservation {
    NotConfigured,
    SettingsSaved,
    PairingRetained,
}

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("run Hosting setup as root")]
    Root,
    #[error("the Anima daemon system account could not be resolved")]
    DaemonUser,
    #[error("Hosting settings or pairing state already exist. Review them locally.")]
    Existing,
    #[error("the system clock is earlier than the Unix epoch")]
    Clock,
    #[error(transparent)]
    Invalid(#[from] SettingsFault),
    #[error(transparent)]
    Store(#[from] Fault),
}

/// Observe the private setup files without opening them or changing state.
///
/// # Errors
/// The caller lacks local authority, or the store or its files cannot be observed safely.
pub fn observe_setup(root: &Path) -> Result<SetupObservation, SetupError> {
    require_root()?;
    let store = Store::observe(root)?;
    let target = root.join(SETTINGS);
    let pair = root.join(PAIR);
    if store.has_private_file(target.to_str().ok_or(Fault::UnsafePlan)?)? {
        return Ok(SetupObservation::SettingsSaved);
    }
    if store.has_private_file(pair.to_str().ok_or(Fault::UnsafePlan)?)? {
        return Ok(SetupObservation::PairingRetained);
    }
    Ok(SetupObservation::NotConfigured)
}

fn require_root() -> Result<(), SetupError> {
    if rustix::process::geteuid().is_root() {
        Ok(())
    } else {
        Err(SetupError::Root)
    }
}

/// Create the first local settings file for the installed daemon and the system Docker engine.
///
/// # Errors
/// The caller lacks local authority, existing configuration is present, or the settings
/// cannot be validated or committed.
pub fn install_system_docker(
    root: &Path,
    (docker, compose): (&str, &str),
) -> Result<(), SetupError> {
    require_root()?;
    let user = nix::unistd::User::from_name("anima-daemon")
        .ok()
        .flatten()
        .filter(|user| !user.uid.is_root())
        .ok_or(SetupError::DaemonUser)?;
    let set_on_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SetupError::Clock)?
        .as_secs();
    let value = document(docker, compose, user.uid.as_raw(), set_on_at);
    let text = format!("{value}\n");
    read(&text)?;
    let target = root.join(SETTINGS);
    let pair = root.join(PAIR);
    let store = Store::open(root)?;
    if store.has_private_file(pair.to_str().ok_or(Fault::UnsafePlan)?)? {
        return Err(SetupError::Existing);
    }
    match store.create(target.to_str().ok_or(Fault::UnsafePlan)?, text.as_bytes()) {
        Err(Fault::Storage(io::ErrorKind::AlreadyExists)) => Err(SetupError::Existing),
        result => result.map_err(SetupError::from),
    }
}

fn document(docker: &str, compose: &str, accepted_user: u32, set_on_at: u64) -> serde_json::Value {
    let defaults = Settings::after_install();
    json!({
        "version": 1,
        "manageApplications": {
            "engine": "system",
            "socket": "/run/docker.sock",
            "docker": docker,
            "compose": compose,
            "acceptedUser": accepted_user,
            "setOnAt": set_on_at,
        },
        "allContainers": defaults.all_containers,
        "openToNetwork": defaults.open_to_network,
        "hubMayDeleteVolumes": defaults.hub_may_delete_volumes,
        "adoptedProjects": [],
        "protected": defaults.protected,
        "registriesWithLogin": defaults.registries,
        "limits": {
            "memoryBytes": defaults.limits.memory_bytes,
            "processes": defaults.limits.processes,
            "processorBillionths": defaults.limits.processor_billionths,
            "stopSeconds": defaults.limits.stop_seconds,
            "freeDiskBytes": defaults.limits.free_disk_bytes,
        },
        "labelPrefixes": defaults.label_prefixes,
    })
}
