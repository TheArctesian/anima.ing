//! Machine-owner repository permission; no broker exposes the mutation request.
mod storage;
use super::{Error, Result};
use daemon_core::{wire::WorkRepository, work_repositories::Repositories};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

const ROOT: &str = "/persist/anima-work-policy";
const MAX_BYTES: usize = 32768;

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Request {
    WorkPolicy,
    WorkPolicySet {
        revision: u64,
        enabled: bool,
        repositories: String,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema: u32,
    revision: u64,
    enabled: bool,
    repositories: String,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            schema: 1,
            revision: 0,
            enabled: true,
            repositories: String::new(),
        }
    }
}

impl Policy {
    fn replace(&self, revision: u64, enabled: bool, repositories: String) -> Result<Self> {
        if self.revision != revision {
            return Err(Error::Refused(
                "The work policy changed. Read it before trying again.",
            ));
        }
        let policy = Self {
            schema: 1,
            revision: revision
                .checked_add(1)
                .ok_or(Error::Refused("The work policy revision is exhausted."))?,
            enabled,
            repositories,
        };
        policy.validate()?;
        Ok(policy)
    }

    fn validate(&self) -> Result<()> {
        if self.schema != 1
            || self.repositories.len() > MAX_BYTES
            || self.repositories.lines().count() > 256
        {
            return Err(Error::Refused(
                "The local work policy exceeds its supported bounds.",
            ));
        }
        Repositories::parse(&self.repositories)
            .map_err(|_| Error::Refused("Use GitHub owner or owner/repository entries."))?;
        Ok(())
    }
}

impl Request {
    pub(super) fn execute(self) -> Result<Value> {
        self.at(Path::new(ROOT))
    }

    fn at(self, root: &Path) -> Result<Value> {
        let _lock = storage::lock(root)?;
        let current = storage::read(root)?;
        match self {
            Self::WorkPolicy => Ok(serde_json::to_value(current)?),
            Self::WorkPolicySet {
                revision,
                enabled,
                repositories,
            } => {
                let policy = current.replace(revision, enabled, repositories)?;
                storage::save(root, &policy)?;
                Ok(serde_json::to_value(policy)?)
            }
        }
    }
}

pub(super) fn authorize(repository: &WorkRepository) -> Result<()> {
    let root = Path::new(ROOT);
    let _lock = storage::lock(root)?;
    let policy = storage::read(root)?;
    if !policy.enabled
        || !Repositories::parse(&policy.repositories)
            .map_err(|_| Error::Refused("The local repository policy is invalid."))?
            .permits(repository)
    {
        return Err(Error::Refused(
            "The local owner does not permit work for this repository.",
        ));
    }
    Ok(())
}

pub(super) fn check_item(item: &Value) -> Result<daemon_core::wire::WorkJob> {
    use daemon_core::{
        wire::WorkKind,
        work::{
            checks::{self, Checked, Machine},
            types::Version,
        },
    };
    let root = Path::new(ROOT);
    let _lock = storage::lock(root)?;
    let policy = storage::read(root)?;
    let repositories = Repositories::parse(&policy.repositories)
        .map_err(|_| Error::Refused("The local repository policy is invalid."))?;
    let machine = Machine {
        switch_on: policy.enabled,
        policy_permits: true,
        kinds: &[WorkKind::Container],
        isolation: &[WorkKind::Container],
        unclean: false,
        repositories: &repositories,
        lowest_runner: Version(0, 0, 0),
    };
    match checks::check(item, &machine) {
        Checked::Start(job) => Ok(*job),
        Checked::Refuse { .. } | Checked::Drop => Err(Error::Refused(
            "The source job does not satisfy the local work policy and request contract.",
        )),
    }
}

#[cfg(test)]
mod tests;
