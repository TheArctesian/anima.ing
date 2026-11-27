//! Identity and integrity receipt shared by the project checkpoint transfer owners.
pub mod socket;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The workspace's maximum extra tar space for path headers and padding.
pub const ARCHIVE_OVERHEAD: u64 = 512 << 20;
pub const MAX_ENTRIES: usize = 50_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub transfer: Uuid,
    pub destination: Uuid,
}

impl Binding {
    #[must_use]
    pub fn fits(self) -> bool {
        !self.transfer.is_nil() && !self.destination.is_nil()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub workspace: Uuid,
    pub generation: u64,
    pub principal: Uuid,
    pub binding: Binding,
}

impl Source {
    #[must_use]
    pub fn fits(self) -> bool {
        !self.workspace.is_nil()
            && !self.principal.is_nil()
            && valid_generation(self.generation)
            && self.binding.fits()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub schema: u32,
    pub source: Source,
    pub digest: String,
    pub bytes: u64,
    pub entries: usize,
}

impl Receipt {
    #[must_use]
    pub fn fits(&self, quota: u64) -> bool {
        self.schema == 1
            && self.source.fits()
            && (1 << 30..=64 << 30).contains(&quota)
            && (1536..=quota.saturating_add(ARCHIVE_OVERHEAD)).contains(&self.bytes)
            && self.bytes.is_multiple_of(512)
            && self.entries <= MAX_ENTRIES
            && self.digest.strip_prefix("sha256:").is_some_and(|hex| {
                hex.len() == 64
                    && hex
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            })
    }
}

/// Binding supplied by the authenticated owner of a destination transfer.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Import {
    pub source_device: Uuid,
    pub destination_device: Uuid,
    pub storage_bytes: u64,
    pub receipt: Receipt,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkout: Option<super::Source>,
}

impl Import {
    #[must_use]
    pub fn fits(&self) -> bool {
        !self.source_device.is_nil()
            && self.source_device != self.destination_device
            && self.destination_device == self.receipt.source.binding.destination
            && self.receipt.fits(self.storage_bytes)
            && valid_generation(self.receipt.source.generation + 1)
            && self.checkout.as_ref().is_none_or(super::Source::fits)
    }
}

#[must_use]
pub fn valid_generation(generation: u64) -> bool {
    (1..=9_007_199_254_740_991).contains(&generation)
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "work", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Offer {
    Export {
        source: Source,
        expires_at: DateTime<Utc>,
        staged: Option<Receipt>,
    },
    Restore {
        import: Import,
        expires_at: DateTime<Utc>,
    },
    Activate {
        import: Import,
        expires_at: DateTime<Utc>,
    },
    Discard {
        import: Import,
        expires_at: DateTime<Utc>,
    },
    Withdraw {
        source: Source,
        expires_at: DateTime<Utc>,
    },
    Retire {
        source: Source,
        expires_at: DateTime<Utc>,
    },
    InspectActivation {
        import: Import,
        expires_at: DateTime<Utc>,
    },
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Withdrawal {
    Discarded {
        import: Import,
    },
    Withdrawn {
        source: Source,
        stopped: Option<super::View>,
    },
}

impl Withdrawal {
    #[must_use]
    pub fn transfer(&self) -> Uuid {
        match self {
            Self::Discarded { import } => import.receipt.source.binding.transfer,
            Self::Withdrawn { source, .. } => source.binding.transfer,
        }
    }
}
