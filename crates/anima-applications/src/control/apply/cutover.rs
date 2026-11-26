//! Verify the running Docker candidate against the installed Compose version.
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

use anima_applications_wire::Health;
use serde::Deserialize;
use zeroize::Zeroizing;

use crate::{
    Paths,
    execution::{files::Store, version},
    final_form::image_by_digest,
    migration::Error,
    plan::Project,
    work::apply::sha256,
};

use super::{
    health,
    observe::{self, ProjectContainer},
    records,
};

const FINAL_FORM_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Deserialize)]
struct Form {
    services: BTreeMap<String, Service>,
}

#[derive(Deserialize)]
struct Service {
    image: String,
    #[serde(default)]
    volumes: Vec<Volume>,
}

#[derive(Deserialize)]
struct Volume {
    #[serde(rename = "type")]
    kind: String,
    source: String,
    target: String,
    #[serde(default)]
    read_only: bool,
}

pub(crate) fn verify_candidate(
    (paths, store): (&Paths, &Store<'_>),
    (application, project): (&str, &str),
    (expected, imported): (&str, &[String]),
    deadline: Instant,
) -> Result<Health, Error> {
    let folders = paths.application(application);
    version::require_settled(store, &folders)?;
    let installed = records::read(store, paths, application)?;
    let installed = installed
        .installed(application)
        .filter(|record| record.has_files)
        .ok_or(Error::CandidateVersion)?;
    store.absent(&folders.dot_env())?;
    let path = folders.final_form();
    let bytes = Zeroizing::new(store.read_small_file(&path, FINAL_FORM_BYTES)?);
    if sha256(&bytes) != expected {
        return Err(Error::CandidateVersion);
    }
    let form: Form = serde_json::from_slice(&bytes).map_err(|_| Error::CandidateVersion)?;
    if form.services.len() != installed.current.len()
        || !installed.current.iter().all(|pulled| {
            form.services.get(&pulled.service).is_some_and(|service| {
                service.image == image_by_digest(&pulled.image, &pulled.digest)
            })
        })
    {
        return Err(Error::CandidateVersion);
    }
    let binds = folders.binds();
    require_imported_binds(store, &form, &binds, imported)?;
    let folder = folders.files();
    let project_files = Project {
        name: project,
        folder: &folder,
        files: std::slice::from_ref(&path),
    };
    let hashes =
        observe::service_hashes(paths, &project_files, deadline)?.ok_or(Error::CandidateVersion)?;
    if !form.services.keys().eq(hashes.keys()) {
        return Err(Error::CandidateVersion);
    }
    let containers = observe::cutover_containers(paths, project, deadline)?;
    if !containers.iter().all(|container| {
        container.folder == folder
            && hashes.get(&container.service) == container.config_hash.as_ref()
            && matches(container, &path, &form)
    }) {
        return Err(Error::CandidateHealth);
    }
    match health::state(
        &containers,
        &folder,
        form.services.keys().map(String::as_str),
    ) {
        Some(Health::Healthy) => Ok(Health::Healthy),
        _ => Err(Error::CandidateHealth),
    }
}

fn require_imported_binds(
    store: &Store<'_>,
    form: &Form,
    root: &str,
    names: &[String],
) -> Result<(), Error> {
    let binds: Vec<_> = form
        .services
        .values()
        .flat_map(|service| &service.volumes)
        .filter(|volume| volume.kind == "bind")
        .collect();
    let sources: BTreeSet<_> = binds.iter().map(|volume| volume.source.as_str()).collect();
    let mut writable = false;
    for name in names {
        let path = format!("{root}/{name}");
        if !sources.contains(path.as_str()) {
            return Err(Error::CandidateHealth);
        }
        store.existing_bind(&path)?;
        writable |= binds
            .iter()
            .any(|volume| volume.source == path && !volume.read_only);
    }
    writable.then_some(()).ok_or(Error::CandidateHealth)
}

fn matches(container: &ProjectContainer, form_path: &str, form: &Form) -> bool {
    let Some(service) = form.services.get(&container.service) else {
        return false;
    };
    container.final_form.as_deref() == Some(form_path)
        && container.image.as_deref() == Some(&service.image)
        && service
            .volumes
            .iter()
            .filter(|volume| volume.kind == "bind")
            .all(|bind| {
                container.mounts.iter().any(|mount| {
                    mount.kind == "bind"
                        && mount.source == bind.source
                        && mount.destination == bind.target
                        && mount.writable != bind.read_only
                })
            })
}
