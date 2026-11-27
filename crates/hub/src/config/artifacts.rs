use super::{
    Error,
    paths::{optional_root, optional_socket},
};
use std::{collections::BTreeMap, path::PathBuf};

const ROOT: &str = "ANIMA_ARTIFACTS_DIR";
const QUOTA: &str = "ANIMA_ARTIFACTS_QUOTA_BYTES";
const RESERVE: &str = "ANIMA_ARTIFACTS_RESERVE_BYTES";
const ORIGIN: &str = "ANIMA_ARTIFACT_REGISTRY_ORIGIN";
const REPOSITORY: &str = "ANIMA_ARTIFACT_REGISTRY_REPOSITORY";
const AUTHORIZATION: &str = "ANIMA_ARTIFACT_REGISTRY_AUTHORIZATION_FILE";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactStorage {
    pub root: PathBuf,
    pub quota: u64,
    pub reserve: u64,
    pub registry: Option<ArtifactRegistry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactRegistry {
    pub origin: String,
    pub repository: String,
    pub authorization: Option<PathBuf>,
}

pub(super) fn from_env(env: &BTreeMap<String, String>) -> Result<Option<ArtifactStorage>, Error> {
    let read = |key| env.get(key).filter(|value| !value.is_empty());
    let root = optional_root(env, ROOT)?;
    let authorization = optional_socket(env, AUTHORIZATION)?;
    let registry = match (read(ORIGIN), read(REPOSITORY), authorization) {
        (None, None, None) => None,
        (Some(origin), Some(repository), authorization) if root.is_some() => {
            Some(ArtifactRegistry {
                origin: origin.clone(),
                repository: repository.clone(),
                authorization,
            })
        }
        _ => return Err(Error::ArtifactRegistry),
    };
    match (root, read(QUOTA), read(RESERVE)) {
        (None, None, None) => Ok(None),
        (Some(root), Some(quota), Some(reserve)) if cfg!(unix) => Ok(Some(ArtifactStorage {
            root,
            quota: positive(quota)?,
            reserve: positive(reserve)?,
            registry,
        })),
        _ => Err(Error::ArtifactStorage),
    }
}

fn positive(text: &str) -> Result<u64, Error> {
    text.parse()
        .ok()
        .filter(|bytes| *bytes > 0)
        .ok_or(Error::ArtifactStorage)
}
