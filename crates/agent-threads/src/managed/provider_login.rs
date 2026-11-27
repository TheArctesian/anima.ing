//! Transient human sign-in authorization; provider challenges are never command receipts.
use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SOCKET: &str = "/run/anima-execution/provider.sock";
pub const MAX_REQUEST: usize = 2048;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Login {
    pub session: Uuid,
    pub principal: Uuid,
    pub workspace: Uuid,
    pub generation: u64,
    pub provider: Provider,
    pub valid_until: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provider {
    Codex,
    Claude,
}

/// One returned OAuth code; never a shell line or arbitrary provider input.
#[must_use]
pub fn valid_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 1024
        && code
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.~#".contains(&byte))
}

impl Login {
    #[must_use]
    pub fn valid(&self, now: DateTime<Utc>) -> bool {
        !self.session.is_nil()
            && !self.principal.is_nil()
            && !self.workspace.is_nil()
            && (1..=9_007_199_254_740_991).contains(&self.generation)
            && self.valid_until > now
            && self.valid_until <= now + TimeDelta::seconds(15)
            && self.expires_at >= self.valid_until
            && self.expires_at <= now + TimeDelta::minutes(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn login(now: DateTime<Utc>) -> Login {
        Login {
            session: Uuid::from_u128(1),
            principal: Uuid::from_u128(2),
            workspace: Uuid::from_u128(3),
            generation: 1,
            provider: Provider::Codex,
            valid_until: now + TimeDelta::seconds(15),
            expires_at: now + TimeDelta::minutes(10),
        }
    }

    #[test]
    fn authorization_cannot_extend_dispatch_or_login_lifetime() {
        let now = DateTime::from_timestamp(1_800_000_000, 0).unwrap();
        let valid = login(now);
        assert!(valid.valid(now));
        assert!(!valid.valid(valid.valid_until));
        let mut late_dispatch = login(now);
        late_dispatch.valid_until += TimeDelta::nanoseconds(1);
        assert!(!late_dispatch.valid(now));
        let mut long_login = login(now);
        long_login.expires_at += TimeDelta::nanoseconds(1);
        assert!(!long_login.valid(now));
        let mut ended = login(now);
        ended.expires_at = ended.valid_until - TimeDelta::nanoseconds(1);
        assert!(!ended.valid(now));
    }

    #[test]
    fn wire_refuses_missing_identity_unknown_provider_and_extra_authority() {
        let now = DateTime::from_timestamp(1_800_000_000, 0).unwrap();
        let value = serde_json::to_value(login(now)).unwrap();
        for field in ["session", "principal", "workspace"] {
            let mut changed = value.clone();
            changed[field] = serde_json::json!(Uuid::nil());
            assert!(!serde_json::from_value::<Login>(changed).unwrap().valid(now));
        }
        for generation in [0, 9_007_199_254_740_992_u64] {
            let mut changed = value.clone();
            changed["generation"] = serde_json::json!(generation);
            assert!(!serde_json::from_value::<Login>(changed).unwrap().valid(now));
        }
        let mut changed = value.clone();
        changed["provider"] = serde_json::json!("shell");
        assert!(serde_json::from_value::<Login>(changed).is_err());
        let mut changed = value;
        changed["executable"] = serde_json::json!("/bin/sh");
        assert!(serde_json::from_value::<Login>(changed).is_err());
    }
}
