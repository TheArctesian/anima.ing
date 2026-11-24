//! The output types of the copies (K1 `ModelCopy`, `HostPlace`, `PullProgress`).
#![expect(
    clippy::unused_async_trait_impl,
    reason = "the async-graphql Enum derive generates async trait impls"
)]

use async_graphql::{Enum, ID, Object, SimpleObject};
use chrono::{DateTime, Utc};
use daemon_core::wire::Progress;

use super::super::found::Found;
use super::super::loads::Load;
use super::super::{CopyState, Listed, Origin};
use super::load::ModelLoad;
use crate::fleet::compute::library::Format;

/// A count as a GraphQL `Float`: exact up to 2^53, which no model size reaches.
#[expect(clippy::cast_precision_loss, reason = "exact below 2^53")]
pub(super) fn float(value: u64) -> f64 {
    value as f64
}

/// The words of a state in a refusal.
pub(super) const fn state_words(state: CopyState) -> &'static str {
    match state {
        CopyState::Queued => "waiting for the machine",
        CopyState::Downloading => "downloading",
        CopyState::Downloaded => "downloaded",
        CopyState::Validating => "validating",
        CopyState::Validated => "validated",
        CopyState::Failed => "failed",
        CopyState::Removing => "being removed",
    }
}

/// The kind of a place where a thing runs: a machine today, a cluster later (HOST-052).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Enum)]
pub enum HostPlaceKind {
    Machine,
}

/// The place where a thing runs (HOST-052): its kind, its identifier and its name.
#[derive(Clone, Debug, SimpleObject)]
pub struct HostPlace {
    pub kind: HostPlaceKind,
    pub id: ID,
    pub name: String,
}

/// The progress of a pull that runs (PULL-007), with the time that the hub computes from it.
#[derive(Clone, Copy, Debug)]
pub struct PullProgress(pub Progress);

#[Object]
impl PullProgress {
    async fn bytes(&self) -> f64 {
        float(self.0.bytes)
    }

    async fn total_bytes(&self) -> f64 {
        float(self.0.total_bytes)
    }

    async fn bytes_per_second(&self) -> f64 {
        float(self.0.bytes_per_second)
    }

    /// The seconds until the pull ends at this rate; null when the rate is 0.
    async fn seconds_left(&self) -> Option<f64> {
        let left = self.0.total_bytes.saturating_sub(self.0.bytes);
        (self.0.bytes_per_second > 0).then(|| float(left.div_ceil(self.0.bytes_per_second)))
    }
}

/// One copy of a variant on one place, with its state now.
pub struct ModelCopyView {
    listed: Listed,
    place: String,
    load: Option<Load>,
}

impl ModelCopyView {
    #[must_use]
    pub const fn new(listed: Listed, place: String) -> Self {
        Self {
            listed,
            place,
            load: None,
        }
    }

    /// The copy with its load, from the last model report of its machine.
    #[must_use]
    pub fn with_load(self, load: Option<Load>) -> Self {
        Self { load, ..self }
    }
}

#[Object(name = "ModelCopy")]
impl ModelCopyView {
    async fn id(&self) -> ID {
        ID(self.listed.copy.id.to_string())
    }

    /// Where the copy is (HOST-052).
    async fn place(&self) -> HostPlace {
        HostPlace {
            kind: HostPlaceKind::Machine,
            id: ID(self.listed.copy.device.to_string()),
            name: self.place.clone(),
        }
    }

    /// `model:tag` (LIBM-006).
    async fn address(&self) -> String {
        self.listed.copy.address()
    }

    async fn model(&self) -> &str {
        &self.listed.copy.model
    }

    async fn tag(&self) -> &str {
        &self.listed.copy.tag
    }

    async fn format(&self) -> Format {
        self.listed.copy.format
    }

    async fn quantisation(&self) -> &str {
        &self.listed.copy.quantisation
    }

    /// `organisation/name` on Hugging Face, at `revision`.
    async fn repository(&self) -> &str {
        &self.listed.copy.repository
    }

    async fn revision(&self) -> &str {
        &self.listed.copy.revision
    }

    async fn size_bytes(&self) -> f64 {
        float(self.listed.copy.size_bytes)
    }

    async fn file_count(&self) -> u32 {
        self.listed.copy.files
    }

    async fn state(&self) -> CopyState {
        self.listed.status.state
    }

    /// Pulled from its source, or found on the machine and used where it is (K11 Section 2.3).
    async fn origin(&self) -> Origin {
        self.listed.copy.origin
    }

    /// The progress of a pull that runs; null in each other state.
    async fn progress(&self) -> Option<PullProgress> {
        self.listed.status.progress.map(PullProgress)
    }

    /// Why the copy failed, or why its removal failed: a fixed code (K11 Section 5) or a fixed
    /// sentence of the hub.
    async fn reason(&self) -> Option<&str> {
        self.listed.status.reason.as_deref()
    }

    /// The copy in the memory of its machine, from the last model report of the machine (K11 Section 11.1); null
    /// when that report did not list it as loaded.
    async fn load(&self) -> Option<ModelLoad> {
        self.load.clone().map(ModelLoad)
    }

    async fn created_at(&self) -> DateTime<Utc> {
        self.listed.copy.created_at
    }
}

/// The program that keeps a found model (K11 Section 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Enum)]
#[graphql(name = "FoundModelTool")]
pub enum FoundTool {
    Ollama,
    LmStudio,
    Huggingface,
    /// A folder that the owner of the machine named in `modelFolders`.
    Folder,
}

impl FoundTool {
    fn of_word(word: &str) -> Self {
        match word {
            "ollama" => Self::Ollama,
            "lm_studio" => Self::LmStudio,
            "huggingface" => Self::Huggingface,
            _ => Self::Folder,
        }
    }
}

/// A model that a machine found where a different program keeps it, with no copy that uses it.
pub struct FoundModelView {
    found: Found,
    place: String,
}

impl FoundModelView {
    #[must_use]
    pub const fn new(found: Found, place: String) -> Self {
        Self { found, place }
    }
}

#[Object(name = "FoundModel")]
impl FoundModelView {
    /// The key that the machine gave the model (K11 Section 2.3); never a path.
    async fn found(&self) -> &str {
        &self.found.entry.found
    }

    /// The machine that found it (HOST-052).
    async fn place(&self) -> HostPlace {
        HostPlace {
            kind: HostPlaceKind::Machine,
            id: ID(self.found.device.to_string()),
            name: self.place.clone(),
        }
    }

    async fn tool(&self) -> FoundTool {
        FoundTool::of_word(&self.found.entry.tool)
    }

    /// The name that its tool shows.
    async fn name(&self) -> &str {
        &self.found.entry.name
    }

    async fn format(&self) -> Format {
        if self.found.entry.format == "gguf" {
            Format::Gguf
        } else {
            Format::Safetensors
        }
    }

    async fn file_count(&self) -> u32 {
        self.found.entry.files
    }

    async fn size_bytes(&self) -> f64 {
        float(self.found.entry.bytes)
    }

    /// Whether a machine can use it where it is: only a GGUF model in this version (MW-P10).
    async fn usable(&self) -> bool {
        self.found.entry.usable
    }

    /// When the machine ended the report that listed it.
    async fn read_at(&self) -> DateTime<Utc> {
        self.found.read_at
    }
}
