//! Bind local cutover authorization to one imported backup and its original Podman store.
mod confirmation;

pub use confirmation::confirm_cutover;

use std::{
    path::Path,
    time::{Duration, Instant},
};

use anima_os_maintenance::cutover::{AUTHORIZATION_BYTES, Format};
use serde::{Deserialize, Serialize};

use crate::{
    Folders,
    execution::{Fault, files::Store},
    forms::is_uuid,
    work::apply::sha256,
};

use super::{Error, Manifest, legacy, local_root};

#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Authorization {
    schema: u32,
    application: String,
    import_id: String,
    manifest_sha256: String,
    source_graph_root: String,
    external_starters_disabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    confirmation: Option<confirmation::Receipt>,
}

/// Record permission to start the Docker candidate after the administrator disables
/// external source starters. The source observation also checks the engine restart policy.
///
/// # Errors
/// Invalid authority or backup, busy storage, active or unverified source, or an existing
/// authorization for a different backup or source location.
pub fn authorize_cutover(
    root: &Path,
    (application, import): (&str, &str),
    confirmation: &std::ffi::OsStr,
) -> Result<Authorization, Error> {
    if confirmation != "--source-startup-disabled" {
        return Err(Error::ExternalStartup);
    }
    let root_text = local_root(root, (application, import))?;
    let paths = Locations::of(root_text, application);
    let observed = Store::observe(root)?;
    let preliminary = Manifest::read(&observed, &paths.manifest, (application, Some(import)))?;
    let _admission = anima_os_maintenance::cutover::authorize(root, preliminary.format())?;
    let store = Store::open(root)?;
    let manifest = Manifest::read(&store, &paths.manifest, (application, Some(import)))?;
    if manifest.format() != preliminary.format() {
        return Err(Error::Cutover);
    }
    let proposed = proof(
        &manifest,
        application,
        Instant::now() + Duration::from_secs(15),
    )?;
    if let Some(recorded) = paths.record(&store, (root_text, application))? {
        return if recorded.matches_source(&proposed) {
            store.sync_record(&paths.authorization)?;
            Ok(recorded)
        } else {
            Err(Error::Cutover)
        };
    }
    store.folder(&format!("{root_text}/cutovers"))?;
    paths.save(&store, &proposed)?;
    Ok(proposed)
}

pub(crate) fn check_start(
    store: &Store<'_>,
    destination: (&Folders, &str),
    deadline: Instant,
) -> Result<(), Fault> {
    checked(store, destination, deadline).map_err(|_| Fault::MigrationHeld)
}

fn checked(
    store: &Store<'_>,
    (folders, socket): (&Folders, &str),
    deadline: Instant,
) -> Result<(), Error> {
    let path = Path::new(folders.application());
    let application = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| is_uuid(name))
        .ok_or(Error::Manifest)?;
    let root = path
        .parent()
        .and_then(Path::to_str)
        .ok_or(Error::Manifest)?;
    let paths = Locations::of(root, application);
    let recorded = paths.record(store, (root, application))?;
    let imported = store.has_private_file(&paths.manifest)?;
    match (imported, recorded) {
        (false, None) => Ok(()),
        (true, Some(recorded)) => {
            if recorded
                .confirmation
                .as_ref()
                .is_some_and(|receipt| !receipt.uses_socket(socket))
            {
                return Err(Error::Cutover);
            }
            let manifest = Manifest::read(
                store,
                &paths.manifest,
                (application, Some(&recorded.import_id)),
            )?;
            recorded.require_source(&manifest, application, deadline)
        }
        _ => Err(Error::Cutover),
    }
}

fn proof(
    manifest: &Manifest,
    application: &str,
    deadline: Instant,
) -> Result<Authorization, Error> {
    Ok(Authorization {
        schema: manifest.format() as u32,
        application: application.to_owned(),
        import_id: manifest.import_id.clone(),
        manifest_sha256: manifest_digest(manifest)?,
        source_graph_root: legacy::stopped(manifest, deadline)?,
        external_starters_disabled: true,
        confirmation: None,
    })
}

fn manifest_digest(manifest: &Manifest) -> Result<String, Error> {
    let bytes = serde_json::to_vec(manifest).map_err(|_| Error::Manifest)?;
    Ok(sha256(&bytes))
}

impl Authorization {
    fn require_source(
        &self,
        manifest: &Manifest,
        application: &str,
        deadline: Instant,
    ) -> Result<(), Error> {
        let expected = proof(
            manifest,
            application,
            deadline.min(Instant::now() + Duration::from_secs(15)),
        )?;
        self.matches_source(&expected)
            .then_some(())
            .ok_or(Error::Cutover)
    }

    fn matches_source(&self, expected: &Self) -> bool {
        self.schema == expected.schema
            && self.application == expected.application
            && self.import_id == expected.import_id
            && self.manifest_sha256 == expected.manifest_sha256
            && self.source_graph_root == expected.source_graph_root
            && self.external_starters_disabled == expected.external_starters_disabled
    }
}

struct Locations {
    manifest: String,
    authorization: String,
}

impl Locations {
    fn of(root: &str, application: &str) -> Self {
        Self {
            manifest: format!("{root}/{application}/migration.json"),
            authorization: format!("{root}/cutovers/{application}.json"),
        }
    }

    fn record(
        &self,
        store: &Store<'_>,
        (root, application): (&str, &str),
    ) -> Result<Option<Authorization>, Error> {
        if !store.has_private_file(&self.authorization)? {
            return Ok(None);
        }
        let bytes = store.read_small_file(&self.authorization, AUTHORIZATION_BYTES)?;
        let record: Authorization = serde_json::from_slice(&bytes).map_err(|_| Error::Cutover)?;
        if Format::from_schema(record.schema).is_none()
            || record.application != application
            || record
                .confirmation
                .as_ref()
                .is_some_and(|receipt| !receipt.matches(root, application))
        {
            return Err(Error::Cutover);
        }
        Ok(Some(record))
    }

    fn save(&self, store: &Store<'_>, authorization: &Authorization) -> Result<(), Error> {
        let bytes = serde_json::to_vec(authorization).map_err(|_| Error::Cutover)?;
        if bytes.len() as u64 > AUTHORIZATION_BYTES {
            return Err(Error::Cutover);
        }
        Ok(store.write(&self.authorization, &bytes)?)
    }
}
