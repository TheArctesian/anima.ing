//! Read digest records without opening another application's files.
use std::io;

use serde::de::DeserializeOwned;

use crate::{
    Folders, Installed, Paths, Pulled, Records,
    execution::{Fault, files::Store, version},
    forms::is_digest,
    inventory::digest_of,
    work::apply::MAX_PULLED_BYTES,
};

pub(in crate::control) fn read(
    store: &Store<'_>,
    paths: &Paths,
    application: &str,
) -> Result<Records, Fault> {
    let folders = paths.application(application);
    version::recover(store, &folders)?;
    let has_files = store.has_private_file(&folders.final_form())?;
    let current = complete_version(store, &folders.current())?;
    let kept = complete_version(store, &folders.kept())?;
    let pulled: Option<Vec<String>> = get(store, &folders.pulled(), MAX_PULLED_BYTES)?;
    let pulled = pulled.unwrap_or_default();
    if !pulled.iter().all(|reference| {
        compose_rules::forms::is_image(reference) && digest_of(reference).is_some_and(is_digest)
    }) {
        return Err(invalid());
    }

    let current = current.unwrap_or_default();
    let applications = (has_files || !pulled.is_empty())
        .then_some(Installed {
            application: application.to_owned(),
            has_files: has_files && !current.is_empty(),
            current,
            kept,
            pulled,
        })
        .into_iter()
        .collect();
    Ok(Records { applications })
}

pub(in crate::control) fn committed(
    store: &Store<'_>,
    folders: &Folders,
) -> Result<Option<Vec<Pulled>>, Fault> {
    version::require_settled(store, folders)?;
    if !store.has_private_file(&folders.final_form())? {
        return Ok(None);
    }
    let current = complete_version(store, &folders.current())?.ok_or(Fault::VersionUnconfirmed)?;
    version::require_settled(store, folders)?;
    Ok(Some(current))
}

fn get<T: DeserializeOwned>(store: &Store<'_>, path: &str, limit: u64) -> Result<Option<T>, Fault> {
    if !store.has_private_file(path)? {
        return Ok(None);
    }
    let bytes = store.read_small_file(path, limit)?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| invalid())
}

const fn invalid() -> Fault {
    Fault::Storage(io::ErrorKind::InvalidData)
}

fn complete_version(
    store: &Store<'_>,
    [compose, environment, images]: &[String; 3],
) -> Result<Option<Vec<Pulled>>, Fault> {
    if !store.has_private_file(compose)? || !store.has_private_file(environment)? {
        return Ok(None);
    }
    digests(store, images)
}

fn digests(store: &Store<'_>, path: &str) -> Result<Option<Vec<Pulled>>, Fault> {
    let record: Option<Vec<Pulled>> = get(store, path, 64 * 1024)?;
    if record.as_ref().is_some_and(|entries| {
        entries.is_empty()
            || entries.len() > 32
            || entries.iter().any(|entry| {
                !compose_rules::forms::is_name(&entry.service)
                    || !compose_rules::forms::is_image(&entry.image)
                    || !is_digest(&entry.digest)
            })
    }) {
        return Err(invalid());
    }
    Ok(record)
}
