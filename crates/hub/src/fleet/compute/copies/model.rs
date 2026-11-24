//! The record of a copy and the state that the screen shows (K11 Section 11, CMP-006).
#![expect(
    clippy::unused_async_trait_impl,
    reason = "the async-graphql Enum derive generates async trait impls"
)]

use chrono::{DateTime, Utc};
use daemon_core::wire::Progress;
use uuid::Uuid;

use super::super::library::Format;
use crate::ids::{CopyId, DeviceId, JobId};

/// One copy of a variant on one machine: what its pull took, and the jobs that pull and
/// remove it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Copy {
    pub id: CopyId,
    pub device: DeviceId,
    /// The variant of the library that a pull took; `None` for a found copy.
    pub variant: Option<Uuid>,
    pub model: String,
    pub tag: String,
    pub format: Format,
    pub quantisation: String,
    /// `organisation/name` on Hugging Face.
    pub repository: String,
    pub revision: String,
    pub size_bytes: u64,
    pub files: u32,
    pub pull_job: JobId,
    pub delete_job: Option<JobId>,
    pub created_at: DateTime<Utc>,
    pub origin: Origin,
    /// The key of the found model (K11 Section 2.3); only for a found copy.
    pub found: Option<String>,
}

impl Copy {
    /// The address of the variant, `model:tag` (LIBM-006); for a found copy, the name that its
    /// tool shows.
    #[must_use]
    pub fn address(&self) -> String {
        match self.origin {
            Origin::Pulled => format!("{}:{}", self.model, self.tag),
            Origin::Found => self.model.clone(),
        }
    }
}

/// How a copy came to its machine: pulled from its source, or found where a different program
/// keeps it and used there (K11 Section 2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, async_graphql::Enum)]
#[graphql(name = "ModelCopyOrigin")]
pub enum Origin {
    Pulled,
    Found,
}

impl Origin {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pulled => "pulled",
            Self::Found => "found",
        }
    }

    #[must_use]
    pub fn from_stored(text: &str) -> Option<Self> {
        [Self::Pulled, Self::Found]
            .into_iter()
            .find(|origin| origin.as_str() == text)
    }
}

/// Where a copy is. `Queued` waits for its machine; `Removing` waits for its `copy_delete`.
/// The others are the states of K11 Section 11.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, async_graphql::Enum)]
#[graphql(name = "ModelCopyState")]
pub enum CopyState {
    Queued,
    Downloading,
    Downloaded,
    Validating,
    Validated,
    Failed,
    Removing,
}

impl CopyState {
    /// The state that a machine reports in `result.model.copyState`.
    #[must_use]
    pub fn from_reported(text: &str) -> Option<Self> {
        match text {
            "downloading" => Some(Self::Downloading),
            "downloaded" => Some(Self::Downloaded),
            "validating" => Some(Self::Validating),
            "validated" => Some(Self::Validated),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

/// The state of a copy now, with the progress of its pull and the reason of a failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub state: CopyState,
    /// The last progress of a pull that runs.
    pub progress: Option<Progress>,
    /// A fixed code of K11 Section 5 or a fixed sentence of the hub; never text of a machine.
    pub reason: Option<String>,
}

/// Where a copy is in the memory of its machine (K11 Section 11.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, async_graphql::Enum)]
#[graphql(name = "ModelLoadState")]
pub enum LoadState {
    Loading,
    Ready,
    Active,
    Unloading,
}

impl LoadState {
    /// The state that a machine reports in `servers[].models[].state`.
    #[must_use]
    pub fn from_reported(text: &str) -> Option<Self> {
        match text {
            "loading" => Some(Self::Loading),
            "ready" => Some(Self::Ready),
            "active" => Some(Self::Active),
            "unloading" => Some(Self::Unloading),
            _ => None,
        }
    }
}
