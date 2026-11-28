//! The body of a claim (K10 Section 4) and the machine that it describes.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{WorkKind, WorkType};

/// The switch at the machine (USE-004). The hub cannot set it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Switch {
    On,
    #[default]
    Off,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkOs {
    Linux,
    Macos,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkArchitecture {
    #[serde(rename = "x86_64")]
    X86_64,
    Aarch64,
}

impl WorkOs {
    /// The word of the wire, also the word of a release source (K11 Section 6).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Linux => "linux",
            Self::Macos => "macos",
        }
    }
}

impl WorkArchitecture {
    /// The word of the wire, also the word of a release source (K11 Section 6).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::X86_64 => "x86_64",
            Self::Aarch64 => "aarch64",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Free {
    pub slots: u32,
    pub memory_bytes: u64,
    pub disk_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Held {
    pub id: Uuid,
    pub attempt: u32,
}

impl Switch {
    #[must_use]
    pub const fn is_off(&self) -> bool {
        matches!(self, Self::Off)
    }
}

/// `POST /work/v1/claim` (K10 Section 4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    #[serde(
        default,
        rename = "buildRecipes",
        skip_serializing_if = "Option::is_none"
    )]
    pub build_recipes: Option<crate::wire::BuildRecipes>,
    pub device: Uuid,
    pub switch: Switch,
    #[serde(default, skip_serializing_if = "Switch::is_off")]
    pub applications: Switch,
    pub types: Vec<WorkType>,
    pub kinds: Vec<WorkKind>,
    pub os: WorkOs,
    pub architecture: WorkArchitecture,
    pub free: Free,
    #[serde(default)]
    pub holds: Vec<Held>,
    #[serde(default)]
    pub unclean: bool,
}
