use agent_threads::managed::workspace::CheckoutGrant;
use chrono::{DateTime, Utc};
use daemon_core::source_jobs::valid_checkout_token;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::{Error, Result};

pub(in crate::linux) struct Credential {
    value: Zeroizing<Vec<u8>>,
    expires_at: DateTime<Utc>,
}

pub(in crate::linux) struct ManagedCredential {
    repository_id: Uuid,
    git: Credential,
}

impl ManagedCredential {
    pub(in crate::linux) fn receive(
        grant: CheckoutGrant,
        value: Zeroizing<Vec<u8>>,
    ) -> Result<Self> {
        if grant.repository_id.is_nil() {
            return Err(Error::Refused("Invalid private checkout repository."));
        }
        Ok(Self {
            repository_id: grant.repository_id,
            git: Credential::new(value, grant.expires_at)?,
        })
    }

    pub(super) const fn repository_id(&self) -> Uuid {
        self.repository_id
    }

    pub(super) fn check(&self) -> Result<()> {
        self.git.seconds().map(drop)
    }

    pub(super) fn into_git(self) -> Credential {
        self.git
    }
}

impl Credential {
    pub(in crate::linux) fn new(
        value: Zeroizing<Vec<u8>>,
        expires_at: DateTime<Utc>,
    ) -> Result<Self> {
        if !valid_checkout_token(&value) {
            return Err(Error::Refused("Invalid source checkout credential."));
        }
        let credential = Self { value, expires_at };
        credential.seconds()?;
        Ok(credential)
    }

    pub(super) fn seconds(&self) -> Result<u64> {
        let seconds = (self.expires_at - Utc::now()).num_seconds();
        u64::try_from(seconds.min(45))
            .ok()
            .filter(|&seconds| seconds > 0)
            .ok_or(Error::Refused("The source checkout credential expired."))
    }

    pub(super) fn git_input(self) -> Zeroizing<Vec<u8>> {
        let prefix = b"username=x-access-token\npassword=";
        let mut input = Zeroizing::new(Vec::with_capacity(prefix.len() + self.value.len() + 2));
        input.extend_from_slice(prefix);
        input.extend_from_slice(&self.value);
        input.extend_from_slice(b"\n\n");
        input
    }
}
