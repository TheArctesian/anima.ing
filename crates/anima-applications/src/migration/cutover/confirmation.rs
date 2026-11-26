//! Commit Docker authority after the owner and operator have verified the candidate.
use std::{
    ffi::OsStr,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use anima_os_maintenance::cutover::{
    APPLICATION_ROOT, authorize, current_generation, valid_system,
};

use crate::{
    control::verify_candidate,
    execution::files::Store,
    forms::{is_lower_hex, is_path, project_of},
    startup::{self, Files, Ready},
};

use super::{Authorization, Error, Locations, Manifest, local_root, manifest_digest};

#[derive(Clone, Copy, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Runtime {
    Docker,
}

#[derive(Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Receipt {
    authoritative_runtime: Runtime,
    generation: Option<String>,
    project: String,
    bind_root: String,
    docker_socket: String,
    final_form_sha256: String,
    health: anima_applications_wire::Health,
    persistent_write_verified: bool,
}

impl Receipt {
    pub(super) fn matches(&self, root: &str, application: &str) -> bool {
        is_lower_hex(&self.final_form_sha256, 64)
            && self.persistent_write_verified
            && self.health == anima_applications_wire::Health::Healthy
            && self.project == project_of(application)
            && self.bind_root == format!("{root}/{application}/binds")
            && is_path(&self.docker_socket)
            && if root == APPLICATION_ROOT {
                self.generation.as_deref().is_some_and(valid_system)
            } else {
                self.generation.is_none()
            }
    }

    pub(super) fn uses_socket(&self, socket: &str) -> bool {
        self.docker_socket == socket
    }
}

/// Record Docker as authoritative for one exact application version.
///
/// # Errors
/// The write was unconfirmed, the source or candidate changed, or storage or boot
/// state could not safely commit the receipt.
pub fn confirm_cutover(
    files: &Files<'_>,
    (application, import): (&str, &str),
    final_form_sha256: &str,
    confirmation: &OsStr,
) -> Result<Authorization, Error> {
    if confirmation != "--verified-persistent-write" {
        return Err(Error::PersistentWrite);
    }
    let root = local_root(files.applications, (application, import))?;
    if root == APPLICATION_ROOT && files.settings != files.applications.join("settings.json") {
        return Err(Error::Authority);
    }
    if !is_lower_hex(final_form_sha256, 64) {
        return Err(Error::CandidateVersion);
    }
    let paths = Locations::of(root, application);
    let store = Store::observe(files.applications)?;
    let (record, manifest) = load(&store, &paths, (root, application, import))?;
    if record.confirmation.is_some() {
        return replay(&store, &paths, record, final_form_sha256);
    }
    let admission = authorize(files.applications, manifest.format())?;
    let ready = startup::check(files)?;
    let generation = admission
        .as_ref()
        .map(|_| current_generation())
        .transpose()?;
    commit(
        (&ready, files.settings),
        &paths,
        (root, application, import),
        (final_form_sha256, generation),
    )
}

fn load(
    store: &Store<'_>,
    paths: &Locations,
    (root, application, import): (&str, &str, &str),
) -> Result<(Authorization, Manifest), Error> {
    let record = paths
        .record(store, (root, application))?
        .ok_or(Error::Cutover)?;
    let manifest = Manifest::read(store, &paths.manifest, (application, Some(import)))?;
    if record.import_id != import
        || record.schema != manifest.format() as u32
        || record.manifest_sha256 != manifest_digest(&manifest)?
        || !record.external_starters_disabled
    {
        return Err(Error::Cutover);
    }
    Ok((record, manifest))
}

fn replay(
    store: &Store<'_>,
    paths: &Locations,
    record: Authorization,
    final_form_sha256: &str,
) -> Result<Authorization, Error> {
    if record
        .confirmation
        .as_ref()
        .is_none_or(|receipt| receipt.final_form_sha256 != final_form_sha256)
    {
        return Err(Error::CandidateVersion);
    }
    store.sync_record(&paths.authorization)?;
    Ok(record)
}

fn commit(
    (ready, settings): (&Ready, &std::path::Path),
    paths: &Locations,
    ids @ (_, application, _): (&str, &str, &str),
    (final_form_sha256, generation): (&str, Option<String>),
) -> Result<Authorization, Error> {
    let executor = ready.executor()?;
    let store = executor.store();
    let (mut record, manifest) = load(store, paths, ids)?;
    if record.confirmation.is_some() {
        return replay(store, paths, record, final_form_sha256);
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    let destination = ready.paths();
    let folders = destination.application(application);
    let project = project_of(application);
    let health = verify_candidate(
        (destination, store),
        (application, &project),
        (final_form_sha256, &manifest.binds),
        deadline,
    )?;
    record.require_source(&manifest, application, deadline)?;
    if !startup::settings_current(settings, ready.settings()) {
        return Err(crate::startup::Error::Refused(crate::start::StartRefusal::Off).into());
    }
    record.confirmation = Some(Receipt {
        authoritative_runtime: Runtime::Docker,
        generation,
        project,
        bind_root: folders.binds(),
        docker_socket: destination.socket.clone(),
        final_form_sha256: final_form_sha256.to_owned(),
        health,
        persistent_write_verified: true,
    });
    paths.save(store, &record)?;
    Ok(record)
}
