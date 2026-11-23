//! Who signs a macOS release (BLD-083). The decision is pure; reading the clone's file is the edge.

use std::{io::Read, path::Path};

use super::mach_o::TeamId;
use crate::builder::{artifact, error::BuilderError};

/// Relative to the clone.
pub const CONFIG: &str = "apps/desktop/src-tauri/tauri.conf.json";
const CONFIG_BYTES_MAX: u64 = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signer {
    pub identity: String,
    pub team: TeamId,
}

impl Signer {
    /// The identity never comes from the clone, which a build can change.
    pub fn pinned(own: Option<&str>, clone: &str) -> Result<Self, BuilderError> {
        match own {
            Some(own) if !own.is_empty() && own == clone => Self::from_identity(own),
            _ => Err(BuilderError::Bundle),
        }
    }

    pub fn from_identity(identity: &str) -> Result<Self, BuilderError> {
        Ok(Self {
            identity: identity.to_owned(),
            team: TeamId::of_identity(identity)?,
        })
    }
}

pub fn configured(config: &serde_json::Value) -> Option<&str> {
    config["bundle"]["macOS"]["signingIdentity"]
        .as_str()
        .filter(|name| !name.is_empty())
}

/// `Bundle` when the file is not the build's own or names none.
pub fn in_clone(clone: &Path) -> Result<String, BuilderError> {
    let file = artifact::open_own(clone, &clone.join(CONFIG))?;
    let config: serde_json::Value =
        serde_json::from_reader(file.take(CONFIG_BYTES_MAX)).map_err(|_| BuilderError::Bundle)?;
    configured(&config)
        .map(str::to_owned)
        .ok_or(BuilderError::Bundle)
}
