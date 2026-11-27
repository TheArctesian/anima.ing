use std::collections::BTreeSet;

use anima_os_maintenance::cutover::Format;
use serde::{Deserialize, Serialize};

use crate::{
    execution::files::Store,
    forms::{is_lower_hex, is_uuid},
};

use super::{Error, legacy::Snapshot};

const LIMIT: u64 = 128 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Manifest {
    schema: u32,
    application: String,
    pub(super) import_id: String,
    backup_id: String,
    pub(super) podman_uid: u32,
    pub(super) podman_containers: Vec<String>,
    pub(super) binds: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<Snapshot>,
}

impl Manifest {
    pub(super) fn read(
        store: &Store<'_>,
        path: &str,
        ids: (&str, Option<&str>),
    ) -> Result<Self, Error> {
        let bytes = store.read_small_file(path, LIMIT)?;
        let mut manifest: Self = serde_json::from_slice(&bytes).map_err(|_| Error::Manifest)?;
        if !manifest.valid(ids) {
            return Err(Error::Manifest);
        }
        manifest.binds.sort_unstable();
        manifest.podman_containers.sort_unstable();
        Ok(manifest)
    }

    pub(super) const fn format(&self) -> Format {
        if self.source.is_some() {
            Format::Captured
        } else {
            Format::Retained
        }
    }

    pub(super) const fn source(&self) -> Option<&Snapshot> {
        self.source.as_ref()
    }

    pub(super) fn record_source(
        &mut self,
        store: &Store<'_>,
        path: &str,
        source: Snapshot,
    ) -> Result<(), Error> {
        if self.source.is_some() || !source.valid(&self.podman_containers) {
            return Err(Error::SourceCapture);
        }
        self.schema = Format::Captured as u32;
        self.source = Some(source);
        let bytes = serde_json::to_vec(self).map_err(|_| Error::Manifest)?;
        if bytes.len() as u64 > LIMIT {
            return Err(Error::Manifest);
        }
        Ok(store.write(path, &bytes)?)
    }

    fn valid(&self, (application, import): (&str, Option<&str>)) -> bool {
        let source_matches = match (Format::from_schema(self.schema), &self.source) {
            (Some(Format::Retained), None) => true,
            (Some(Format::Captured), Some(source)) => source.valid(&self.podman_containers),
            _ => false,
        };
        source_matches
            && self.application == application
            && identity(&self.import_id)
            && import.is_none_or(|expected| self.import_id == expected)
            && identity(&self.backup_id)
            && self.backup_id != self.import_id
            && (1..=128).contains(&self.podman_containers.len())
            && self.podman_containers.iter().all(|id| is_lower_hex(id, 64))
            && unique(&self.podman_containers)
            && (1..=64).contains(&self.binds.len())
            && self
                .binds
                .iter()
                .all(|name| compose_rules::forms::is_name(name))
            && unique(&self.binds)
    }
}

fn unique(names: &[String]) -> bool {
    names.iter().collect::<BTreeSet<_>>().len() == names.len()
}

pub(super) fn identity(text: &str) -> bool {
    is_uuid(text) && text != "00000000-0000-0000-0000-000000000000"
}
