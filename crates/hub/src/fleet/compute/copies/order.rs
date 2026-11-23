//! The model items of a copy: `model_pull` and `copy_delete` for one machine (K11 Sections 2
//! and 7). The hub checks the files by the rules of the machine before it asks, so a pull
//! that a machine would refuse for its files is refused here first.

use daemon_core::{
    model_work::{
        files::{self, MAX_FILES},
        names::{self, Actor, is_lower_hex, is_source_name},
    },
    wire::{Limits, WorkKind, WorkType},
};
use serde_json::{Value, json};

use super::super::library::{Format, PullSource};
use crate::{
    fleet::events::ActorKind,
    ids::{CopyId, DeviceId},
    work::jobs::{NewJob, Target},
};

const GIB: u64 = 1024 * 1024 * 1024;

/// A pull may take a day; one with no report for an hour has stopped (K10 Section 7).
const PULL_LIMITS: Limits = Limits {
    seconds: 24 * 3600,
    idle_seconds: 3600,
    memory_bytes: GIB,
    disk_bytes: 1,
    orphan_seconds: 600,
};

const DELETE_LIMITS: Limits = Limits {
    seconds: 600,
    idle_seconds: 600,
    memory_bytes: GIB,
    disk_bytes: 1,
    orphan_seconds: 600,
};

/// The actor of a model item: who asked (K11 Section 3).
#[must_use]
pub const fn actor_of(kind: ActorKind) -> Actor {
    match kind {
        ActorKind::User => Actor::User,
        ActorKind::Agent => Actor::Agent,
        ActorKind::Hub => Actor::Hub,
    }
}

const fn format_of(format: Format) -> names::Format {
    match format {
        Format::Gguf => names::Format::Gguf,
        Format::Mlx => names::Format::Mlx,
        Format::Safetensors => names::Format::Safetensors,
    }
}

/// Why the files of a variant cannot be pulled, in words for the operator; `None` when they can.
#[must_use]
pub fn unpullable(source: &PullSource) -> Option<&'static str> {
    let variant = &source.variant;
    let format = format_of(variant.format);
    let parts = variant.repository.split_once('/');
    let named = parts.is_some_and(|(organisation, name)| {
        is_source_name(organisation) && is_source_name(name) && is_lower_hex(&variant.revision, 40)
    });
    if !named {
        return Some("Its repository or its commit is not one that a machine takes.");
    }
    if variant.files.is_empty() || variant.files.len() > MAX_FILES {
        return Some("It has no file, or more than a machine takes.");
    }
    let each_fits = variant.files.iter().all(|file| {
        files::is_file_path(&file.path)
            && files::permitted(format, &file.path)
            && file.size_bytes > 0
            && is_lower_hex(&file.sha256, 64)
    });
    if !each_fits {
        return Some("A file of it is of a type or a name that a machine refuses.");
    }
    let mut flat: Vec<String> = variant
        .files
        .iter()
        .map(|file| files::flat_name(&file.path).to_ascii_lowercase())
        .collect();
    flat.sort_unstable();
    flat.dedup();
    (flat.len() != variant.files.len())
        .then_some("Two of its files have one name in the folder of a copy.")
}

fn model_job(
    work_type: WorkType,
    (machine, actor): (DeviceId, Actor),
    parameters: Value,
) -> NewJob {
    NewJob {
        work_type,
        kind: WorkKind::Model,
        os: None,
        architecture: None,
        project: None,
        target: Target::Model { actor },
        parameters,
        secrets: Vec::new(),
        limits: DELETE_LIMITS,
        machine: Some(machine),
    }
}

/// The `model_pull` of `copy` on `machine`. A gated repository is never in the library (it
/// reads only public ones), so `gated` is false and the item has no secret (PULL-010).
#[must_use]
pub fn pull(copy: CopyId, source: &PullSource, (machine, actor): (DeviceId, Actor)) -> NewJob {
    let variant = &source.variant;
    let (organisation, repository) = variant
        .repository
        .split_once('/')
        .unwrap_or((variant.repository.as_str(), ""));
    let files: Vec<Value> = variant
        .files
        .iter()
        .map(|file| json!({ "name": file.path, "sizeBytes": file.size_bytes, "sha256": file.sha256 }))
        .collect();
    let parameters = json!({
        "copy": copy,
        "variant": source.variant_id,
        "source": { "organisation": organisation, "repository": repository, "revision": variant.revision },
        "format": variant.format.as_str(),
        "files": files,
        "gated": false,
    });
    NewJob {
        limits: Limits {
            disk_bytes: variant.size_bytes().max(1),
            ..PULL_LIMITS
        },
        ..model_job(WorkType::ModelPull, (machine, actor), parameters)
    }
}

/// The `copy_delete` of `copy` on `machine` (PULL-009).
#[must_use]
pub fn delete(copy: CopyId, (machine, actor): (DeviceId, Actor)) -> NewJob {
    model_job(
        WorkType::CopyDelete,
        (machine, actor),
        json!({ "copy": copy }),
    )
}
