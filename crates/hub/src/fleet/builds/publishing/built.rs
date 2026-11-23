//! One release of one target that the release workflow built (R-179), checked with no queue and
//! no database (BLD-002, BLD-046, BLD-064 to BLD-069). A refusal names files and versions only;
//! no key, signature, token or URL reaches an error (LOG-007).

mod checks;
mod command;
mod publisher;
mod record;

use bytes::Bytes;

pub use checks::{CheckedBuilt, check_built, sign_built};
pub use command::{ReleaseError, release_command};
pub use publisher::BuiltPublisher;

use super::transfer::Stop;
use crate::fleet::builds::{model::Target, version::Version};

pub struct BuiltFile {
    pub name: String,
    pub bytes: Bytes,
}

pub struct Built {
    pub target: Target,
    pub version: Version,
    pub files: Vec<BuiltFile>,
}

#[derive(Debug, thiserror::Error)]
pub enum BuiltError {
    /// Nothing was written after the failed check, and no feed.
    #[error("{0}")]
    Refused(String),
    /// The same command can run again.
    #[error("the release bucket or the public address cannot be reached ({0})")]
    Unavailable(&'static str),
}

impl From<Stop> for BuiltError {
    fn from(stop: Stop) -> Self {
        match stop {
            Stop::Refused(reason) => Self::Refused(reason),
            Stop::Unavailable(fault) => Self::Unavailable(fault),
            Stop::TakenOver => Self::Unavailable("hold"),
        }
    }
}

fn refused(reason: String) -> BuiltError {
    BuiltError::Refused(reason)
}
