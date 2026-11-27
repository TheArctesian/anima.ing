//! Correlate visible Compose containers with committed local application records.
use std::{collections::BTreeMap, io::Read, path::Path};

use serde::Deserialize;

use crate::{
    Folders,
    execution::{Fault, files::Store},
    final_form::image_by_digest,
    forms::is_lower_hex,
};

use super::{apply::records, container};

const MAX_BYTES: u64 = 1_048_576;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Observation {
    id: String,
    project: String,
    service: String,
    folder: String,
    final_form: String,
    configured_image: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Local administrator authorization is required.")]
    Owner,
    #[error("The container observations are invalid or exceed the inventory limit.")]
    Observations,
    #[error(transparent)]
    Store(#[from] Fault),
}

/// Classify reported Compose services against settled records in the local application store.
/// A null entry denotes an unavailable observation for that container.
///
/// # Errors
/// The caller lacks local authority, the root store is unavailable, or the input is invalid.
pub fn classify(root: &Path, input: impl Read) -> Result<BTreeMap<String, Option<String>>, Error> {
    if !rustix::process::geteuid().is_root() {
        return Err(Error::Owner);
    }
    let observed = observations(input)?;
    let store = Store::observe(root)?;
    let root = root.to_str().ok_or(Fault::UnsafePlan)?;
    let mut records = BTreeMap::new();
    let mut found = BTreeMap::new();
    for observed in observed {
        let Some(application) = container::application(&observed.project) else {
            continue;
        };
        let folders = Folders::at(root, &application);
        if !observed.uses(&folders) {
            continue;
        }
        let record = records
            .entry(application.clone())
            .or_insert_with(|| records::committed(&store, &folders));
        let membership = match record {
            Ok(Some(current)) => {
                let Some(image) = current
                    .iter()
                    .find(|image| image.service == observed.service)
                else {
                    continue;
                };
                match observed.configured_image.as_deref() {
                    Some(reference)
                        if reference == image_by_digest(&image.image, &image.digest) =>
                    {
                        Some(application)
                    }
                    Some(_) => continue,
                    None => None,
                }
            }
            Ok(None) => continue,
            Err(_) => None,
        };
        found.insert(observed.id, membership);
    }
    Ok(found)
}

impl Observation {
    fn uses(&self, folders: &Folders) -> bool {
        is_lower_hex(&self.id, 64)
            && compose_rules::forms::is_name(&self.service)
            && self.folder == folders.files()
            && self.final_form == folders.final_form()
    }
}

fn observations(input: impl Read) -> Result<Vec<Observation>, Error> {
    let mut bytes = Vec::new();
    let count = input
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Observations)?;
    if count as u64 > MAX_BYTES {
        return Err(Error::Observations);
    }
    serde_json::from_slice(&bytes).map_err(|_| Error::Observations)
}
