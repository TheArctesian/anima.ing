//! A workspace preview is a leased namespace endpoint, never a host address.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SOCKET: &str = "/run/anima-execution/preview.sock";
pub const MAX_REQUEST: usize = 4096;
pub const FORWARD_PATH: &str = "/.anima/forward";
pub const FORWARD_PROTOCOL: &str = "anima-workspace-forward.v1";
pub const FORWARD_CHUNK: usize = 16 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ForwardSession {
    pub token: String,
    pub seconds: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub endpoint: Uuid,
    pub principal: Uuid,
    pub grant: Uuid,
    pub workspace: Uuid,
    pub generation: u64,
    pub port: u16,
    pub expires_at: u64,
}
/// The enrolled transport has already checked the hub's principal and grant.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Open {
    pub binding: Binding,
    pub valid_until: chrono::DateTime<chrono::Utc>,
}

impl Open {
    #[must_use]
    pub fn valid(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        let binding = &self.binding;
        let Ok(seconds) = u64::try_from(now.timestamp()) else {
            return false;
        };
        ![
            binding.endpoint,
            binding.principal,
            binding.grant,
            binding.workspace,
        ]
        .iter()
        .any(Uuid::is_nil)
            && (1..=9_007_199_254_740_991).contains(&binding.generation)
            && binding.port != 0
            && binding.expires_at > seconds
            && binding.expires_at <= seconds.saturating_add(600)
            && self.valid_until > now
            && self.valid_until <= now + chrono::TimeDelta::seconds(15)
    }
}
