//! Receive a prepared legacy backup at the application owner's bind boundary.
mod compose;
mod cutover;
mod legacy;
mod manifest;

pub use compose::{ComposeInputs, prepare_compose};
pub(crate) use cutover::check_start;
pub use cutover::{Authorization, authorize_cutover, confirm_cutover};

use std::{
    io,
    path::Path,
    time::{Duration, Instant},
};

use crate::{
    execution::{Fault, files::Store},
    forms::is_path,
};
use manifest::{Manifest, identity};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Maintenance(#[from] anima_os_maintenance::Error),
    #[error(transparent)]
    Startup(#[from] crate::startup::Error),
    #[error("the import requires root and valid application and import UUIDs")]
    Authority,
    #[error("the prepared import manifest is invalid or does not match the request")]
    Manifest,
    #[error("the legacy Compose input or bind map could not be read within its limit")]
    ComposeInput,
    #[error(
        "the bind map must cover every imported directory and every legacy mount source exactly"
    )]
    ComposeBinds,
    #[error(
        "legacy mounts must be ordinary named volumes or directory binds with a read-only or read-write mode"
    )]
    ComposeMount,
    #[error("legacy ports must use fixed host and container ports and a supported protocol")]
    ComposePort,
    #[error("the legacy Compose document needs manual translation before import")]
    ComposeShape,
    #[error("the prepared Compose document cannot be encoded")]
    ComposeOutput,
    #[error(transparent)]
    ComposeRule(#[from] compose_rules::Refusal),
    #[error("the application directory already exists with another identity")]
    TargetExists,
    #[error(
        "the source containers or runtime could not be verified as stopped with restart disabled"
    )]
    SourceActive,
    #[error(
        "capture the exact source names and systemd launchers before the Podman containers are removed"
    )]
    SourceCapture,
    #[error("each observed Podman systemd launcher must be inactive and persistently masked")]
    SourceLauncher,
    #[error("confirm that the original workload's external starters have been disabled")]
    ExternalStartup,
    #[error(
        "the recorded cutover authorization does not match the imported backup or source runtime"
    )]
    Cutover,
    #[error(
        "verify a persistent application write on the Docker candidate before confirming cutover"
    )]
    PersistentWrite,
    #[error("the confirmed Compose version does not match the requested final form")]
    CandidateVersion,
    #[error(
        "the Docker candidate is not healthy or its running configuration, image or transferred mounts do not match"
    )]
    CandidateHealth,
    #[error("the application store or prepared import could not be verified: {0}")]
    Storage(#[from] Fault),
}

#[derive(serde::Serialize)]
pub struct Receipt {
    state: &'static str,
    #[serde(flatten)]
    manifest: Manifest,
}

/// Record the identities needed for cutover when Podman removes a container on stop.
///
/// # Errors
/// Invalid authority or staging state, unavailable Podman source, or an auto-remove
/// container without a verifiable launcher.
pub fn capture_source(root: &Path, (application, import): (&str, &str)) -> Result<Receipt, Error> {
    let root_text = local_root(root, (application, import))?;
    let source = format!("{root_text}/imports/{import}");
    let record = format!("{source}/migration.json");
    let observed = Store::observe(root)?;
    let _folder = observed.lock_import(&source)?;
    let mut manifest = Manifest::read(&observed, &record, (application, Some(import)))?;
    if manifest.source().is_none() {
        let snapshot = legacy::capture(&manifest, Instant::now() + Duration::from_secs(15))?;
        let writer = Store::open(root)?;
        manifest.record_source(&writer, &record, snapshot)?;
    } else {
        observed.sync_record(&record)?;
    }
    Ok(Receipt {
        state: "source-captured",
        manifest,
    })
}

/// Install a root-prepared set of independent binds on the application's filesystem.
///
/// # Errors
/// Invalid authority, manifest mismatch, unsafe or busy storage, or a conflicting application.
pub fn import_binds(root: &Path, application: &str, import: &str) -> Result<Receipt, Error> {
    let root_text = local_root(root, (application, import))?;
    let source = format!("{root_text}/imports/{import}");
    let target = format!("{root_text}/{application}");
    let paths = (source.as_str(), target.as_str());
    let observed = Store::observe(root)?;
    if let Some(receipt) = installed(&observed, paths, (application, import))? {
        return Ok(receipt);
    }
    let folder = match observed.lock_import(&source) {
        Err(fault @ Fault::Storage(io::ErrorKind::NotFound)) => {
            return installed(&observed, paths, (application, import))?.ok_or(fault.into());
        }
        result => result?,
    };
    let record = format!("{source}/migration.json");
    let manifest = Manifest::read(&observed, &record, (application, Some(import)))?;
    let binds = observed.inspect_import(&folder, &record)?;
    if binds != manifest.binds {
        return Err(Error::Manifest);
    }
    let writer = Store::open(root)?;
    match writer.install_import(paths, &folder) {
        Err(Fault::Storage(io::ErrorKind::AlreadyExists)) => Err(Error::TargetExists),
        other => other.map(|()| receipt(manifest)).map_err(Error::from),
    }
}

fn local_root<'a>(root: &'a Path, (application, import): (&str, &str)) -> Result<&'a str, Error> {
    if !rustix::process::geteuid().is_root() || !identity(application) || !identity(import) {
        return Err(Error::Authority);
    }
    root.to_str()
        .filter(|path| is_path(path))
        .ok_or(Error::Authority)
}

fn installed(
    store: &Store<'_>,
    paths: (&str, &str),
    (application, import): (&str, &str),
) -> Result<Option<Receipt>, Error> {
    let record = format!("{}/migration.json", paths.1);
    if !store.has_private_file(&record)? {
        return Ok(None);
    }
    let manifest = Manifest::read(store, &record, (application, Some(import))).map_err(
        |error| match error {
            Error::Manifest => Error::TargetExists,
            other => other,
        },
    )?;
    store.sync_import(paths)?;
    Ok(Some(receipt(manifest)))
}

const fn receipt(manifest: Manifest) -> Receipt {
    Receipt {
        state: "imported",
        manifest,
    }
}
