//! No type here holds a key's value; it is a
//! [`Secret`](crate::fleet::credentials::Secret) until it is sealed (LOG-007).

use chrono::{DateTime, Utc};

use crate::ids::{DeviceId, OperationalKeyId};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Metadata {
    pub provider: String,
    pub purpose: String,
    pub instructions: String,
    pub provider_link: Option<String>,
    pub rotation_days: Option<i32>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// A version that ended is never active again (K-E5 Section 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VersionState {
    Active,
    Superseded,
    Revoked,
    Expired,
}

impl VersionState {
    #[must_use]
    pub const fn as_sql(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Superseded => "superseded",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }

    #[must_use]
    pub fn from_sql(text: &str) -> Option<Self> {
        [Self::Active, Self::Superseded, Self::Revoked, Self::Expired]
            .into_iter()
            .find(|state| state.as_sql() == text)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Due {
    Current,
    Due,
    Expiring,
    Expired,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub number: i32,
    pub state: VersionState,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub ended_at: Option<DateTime<Utc>>,
    /// As the operator reported it; the hub does not check the provider.
    pub rotated_at_provider: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Consumer {
    pub device: DeviceId,
    pub workload: String,
    pub granted_at: DateTime<Utc>,
    pub granted_by: String,
    pub held_version: Option<i32>,
    pub held_seen_at: Option<DateTime<Utc>>,
    pub fetched_version: Option<i32>,
    pub fetched_at: Option<DateTime<Utc>>,
}

/// Versions are newest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationalKey {
    pub id: OperationalKeyId,
    pub name: String,
    pub metadata: Metadata,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub versions: Vec<Version>,
    pub consumers: Vec<Consumer>,
    pub due: Due,
}

impl OperationalKey {
    #[must_use]
    pub fn active_version(&self) -> Option<i32> {
        self.versions
            .iter()
            .find(|version| version.state == VersionState::Active)
            .map(|version| version.number)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ending {
    pub key: OperationalKey,
    pub ended: Option<i32>,
    pub still_held_by: Vec<Consumer>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grant {
    pub name: String,
    pub device: DeviceId,
    pub workload: String,
}
