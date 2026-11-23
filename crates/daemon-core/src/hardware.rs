//! The hardware values of a device (CAP-001 to CAP-005, CAP-032).
//! Pure: the daemon reads the files and runs the fixed commands (CAP-032, CMP-005).

mod backends;
pub mod parse;
pub mod power;
#[cfg(test)]
mod tests;

pub use backends::{Backend, BackendName, BackendReason, Facts, KfdAccess, backends};

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::inventory::is_tool_name;

pub const MAX_CPU_FEATURES: usize = 32;

pub const MAX_BACKENDS: usize = 16;

/// The largest byte count a batch may carry: the hub stores it as a `bigint`.
pub const MAX_BYTES: u64 = i64::MAX as u64;

/// The `hardware` member of a batch; it replaces the stored values whole (K8 Section 4).
/// An absent value is one the daemon could not read, never zero.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hardware {
    pub system_memory_bytes: u64,
    /// The memory the CPU and the GPU share: Apple silicon only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unified_memory_bytes: Option<u64>,
    /// The sum of the video memory of the GPUs that have their own (NVIDIA, AMD).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dedicated_memory_bytes: Option<u64>,
    /// Apple silicon: the GPU working-set limit of macOS (CAP-005).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usable_memory: Option<UsableMemory>,
    /// The free space of the file system of the daemon's home folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_disk_bytes: Option<u64>,
    pub cpu_features: Vec<String>,
    pub backends: Vec<Backend>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_source: Option<PowerSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thermal_state: Option<ThermalState>,
    /// Whether a person used the keyboard or the pointer in the last 5 minutes (macOS only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_active: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsableMemory {
    pub bytes: u64,
    pub source: UsableSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UsableSource {
    /// The operator set `iogpu.wired_limit_mb`; its value is the limit.
    WiredLimit,
    /// `iogpu.wired_limit_mb` is 0: the share macOS gives ([`parse::usable_memory`]).
    DefaultShare,
    /// A source a newer daemon knows; the hub drops the value.
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PowerSource {
    Mains,
    Battery,
    /// A source a newer daemon knows; the hub drops the value.
    #[serde(other)]
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThermalState {
    Nominal,
    Throttled,
    /// A state a newer daemon knows; the hub drops the value.
    #[serde(other)]
    Unknown,
}

impl Hardware {
    /// The values the hub stores, with unknown names, sources and states dropped; `None` when
    /// the member breaks K8 Section 4.
    #[must_use]
    pub fn accepted(mut self) -> Option<Self> {
        let bytes = [
            Some(self.system_memory_bytes),
            self.unified_memory_bytes,
            self.dedicated_memory_bytes,
            self.usable_memory.map(|usable| usable.bytes),
            self.free_disk_bytes,
        ];
        let features: BTreeSet<&str> = self.cpu_features.iter().map(String::as_str).collect();
        let names: BTreeSet<BackendName> = self.backends.iter().map(|one| one.name).collect();
        let fits = bytes.iter().flatten().all(|value| *value <= MAX_BYTES)
            && self.cpu_features.len() <= MAX_CPU_FEATURES
            && features.len() == self.cpu_features.len()
            && features.iter().all(|feature| is_tool_name(feature))
            && self.backends.len() <= MAX_BACKENDS
            && names.len() == self.backends.len();
        if !fits {
            return None;
        }
        self.backends.retain(|one| one.name != BackendName::Other);
        self.backends.sort_by_key(|one| one.name);
        self.cpu_features.sort_unstable();
        self.usable_memory = self
            .usable_memory
            .filter(|usable| usable.source != UsableSource::Unknown);
        self.power_source = self.power_source.filter(|s| *s != PowerSource::Unknown);
        self.thermal_state = self.thermal_state.filter(|s| *s != ThermalState::Unknown);
        Some(self)
    }
}

impl UsableSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WiredLimit => "wired_limit",
            Self::DefaultShare => "default_share",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Self {
        [Self::WiredLimit, Self::DefaultShare]
            .into_iter()
            .find(|one| one.as_str() == text)
            .unwrap_or(Self::Unknown)
    }
}

impl PowerSource {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Mains => "mains",
            Self::Battery => "battery",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Self {
        [Self::Mains, Self::Battery]
            .into_iter()
            .find(|one| one.as_str() == text)
            .unwrap_or(Self::Unknown)
    }
}

impl ThermalState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Nominal => "nominal",
            Self::Throttled => "throttled",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub fn parse(text: &str) -> Self {
        [Self::Nominal, Self::Throttled]
            .into_iter()
            .find(|one| one.as_str() == text)
            .unwrap_or(Self::Unknown)
    }
}
