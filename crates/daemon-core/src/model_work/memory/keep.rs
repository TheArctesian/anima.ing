//! The idle time and the pin of a copy, and its reservation (K11 Section 11.1, FIT-022 to FIT-024).

use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use uuid::Uuid;

use super::returns::{FIRST_INTERVAL, LONGEST_INTERVAL};
use crate::model_work::names::Runtime;
use crate::model_work::settings::{self, ServerSettings, SettingLimits};

/// The permitted idle times, in seconds.
pub const IDLE_SECONDS: RangeInclusive<u32> = 60..=86_400;
/// The idle time of a copy that the node holds no value for: 15 minutes.
pub const DEFAULT_IDLE_SECONDS: u32 = 900;
/// The largest `reserveBytes`: the largest integer that JSON keeps exact.
pub const MAX_RESERVE_BYTES: u64 = (1 << 53) - 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Keep {
    pub idle_seconds: u32,
    pub pinned: bool,
}

impl Default for Keep {
    fn default() -> Self {
        Self {
            idle_seconds: DEFAULT_IDLE_SECONDS,
            pinned: false,
        }
    }
}

/// The reservation of a copy: the larger of the hub's `reserve_bytes` and the daemon's own lower estimate,
/// which is the files and a quarter more, and at least `share_floor` (the share of a GPU that vLLM takes).
/// A wrong hub can make it larger, never smaller.
#[must_use]
pub fn reservation(files_bytes: u64, reserve_bytes: Option<u64>, share_floor: u64) -> u64 {
    let own = files_bytes.saturating_add(files_bytes / 4).max(share_floor);
    reserve_bytes.map_or(own, |hub| hub.max(own))
}

/// The most bytes of a keep file, and the most keep files of a node.
pub const MAX_FILE_BYTES: u64 = 4096;
pub const MAX_FILES: usize = 64;

/// What the node keeps for a copy in `<model-state>/keep/<copy>.json` (Section 11.1). The clock is Unix seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Kept {
    pub keep: Keep,
    pub displaced: bool,
    /// The server and the settings of the last load.
    pub server: Option<Uuid>,
    pub settings: Map<String, Value>,
    pub reserve_bytes: Option<u64>,
    pub returned_at: Option<u64>,
    pub interval: u64,
}

impl Kept {
    /// The values of a copy with no file.
    #[must_use]
    pub fn new(keep: Keep) -> Self {
        Self {
            keep,
            displaced: false,
            server: None,
            settings: Map::new(),
            reserve_bytes: None,
            returned_at: None,
            interval: FIRST_INTERVAL,
        }
    }

    /// The settings of the last load, read again by the rules of Section 2.1.
    #[must_use]
    pub fn load_settings(&self) -> Option<ServerSettings> {
        let limits = SettingLimits {
            memory_share: 1.0,
            gpus: &[],
        };
        settings::read(&self.settings, Some(Runtime::LlamaServer), limits).ok()
    }

    /// A file of the daemon's form; else the daemon deletes it and uses the defaults.
    #[must_use]
    pub fn read(bytes: &[u8]) -> Option<Self> {
        let kept: Self = serde_json::from_slice(bytes).ok()?;
        let holds = IDLE_SECONDS.contains(&kept.keep.idle_seconds)
            && kept
                .reserve_bytes
                .is_none_or(|bytes| (1..=MAX_RESERVE_BYTES).contains(&bytes))
            && (FIRST_INTERVAL..=LONGEST_INTERVAL).contains(&kept.interval)
            && kept.load_settings().is_some();
        holds.then_some(kept)
    }
}
