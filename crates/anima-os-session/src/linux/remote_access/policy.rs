//! Provenance and private-link port publication for the installed node.
use super::{Error, Result, command};
use nix::unistd::{Gid, Uid};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{fs, io::Read, os::unix::fs::OpenOptionsExt, path::Path};

pub(super) const DEFAULTS: [u16; 3] = [22, 3389, 5900];

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Origin {
    PendingDefaults,
    ManagedDefaults,
    LegacyPreserved,
    OwnerSelected,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Policy {
    schema: u32,
    pub(super) origin: Origin,
    pub(super) baseline: Vec<u16>,
}

pub(super) struct Storage<'a> {
    policy: &'a Path,
    ports: &'a Path,
    staging: &'a Path,
}

impl Storage<'static> {
    pub(super) fn live() -> Self {
        Self {
            policy: Path::new("/persist/anima-remote-access.json"),
            ports: Path::new("/var/lib/anima-link/open-ports"),
            staging: Path::new("/var/lib/anima-remote-defaults"),
        }
    }
}

impl Storage<'_> {
    pub(super) fn policy(&self) -> Result<Option<Policy>> {
        let policy = read::<Policy>(self.policy)?;
        if policy.as_ref().is_some_and(|policy| policy.schema != 1) {
            return Err(Error::Refused(
                "Remote access settings need a compatible OS version.",
            ));
        }
        Ok(policy)
    }

    pub(super) fn ports(&self) -> Result<Option<Vec<u16>>> {
        let Some(mut ports) = read::<Vec<u16>>(self.ports)? else {
            return Ok(None);
        };
        if ports.iter().any(|port| !DEFAULTS.contains(port)) {
            return Err(Error::Refused(
                "Remote access settings contain an unsupported port.",
            ));
        }
        ports.sort_unstable();
        ports.dedup();
        Ok(Some(ports))
    }

    pub(super) fn record_owner_choice(&self) -> Result<()> {
        let ports = self
            .ports()?
            .ok_or(Error::Refused("Remote access choices were not saved."))?;
        self.save(Origin::OwnerSelected, ports)
    }

    pub(super) fn initialize(&self, owner: (Uid, Gid)) -> Result<()> {
        if let Some(policy) = self.policy()? {
            return if policy.origin == Origin::PendingDefaults {
                self.finish_defaults(owner)
            } else {
                Ok(())
            };
        }
        if let Some(existing) = self.ports()? {
            return self.save(Origin::LegacyPreserved, existing);
        }
        self.save(Origin::PendingDefaults, DEFAULTS.to_vec())?;
        self.finish_defaults(owner)
    }

    fn finish_defaults(&self, owner: (Uid, Gid)) -> Result<()> {
        let staged = self.staging.join("open-ports");
        if self.ports()?.is_none() {
            self.publish_defaults(&staged, owner)?;
        }
        sync_parent(self.ports)?;
        match fs::remove_file(&staged) {
            Ok(()) => sync_parent(&staged)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
        self.save(Origin::ManagedDefaults, DEFAULTS.to_vec())
    }

    fn publish_defaults(&self, staged: &Path, (uid, gid): (Uid, Gid)) -> Result<()> {
        command::atomic(staged, &serde_json::to_vec(&DEFAULTS)?, 0o600)?;
        let file = fs::File::open(staged)?;
        nix::unistd::fchown(&file, Some(uid), Some(gid)).map_err(std::io::Error::from)?;
        file.sync_all()?;
        match fs::hard_link(staged, self.ports) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

    fn save(&self, origin: Origin, baseline: Vec<u16>) -> Result<()> {
        let bytes = serde_json::to_vec(&Policy {
            schema: 1,
            origin,
            baseline,
        })?;
        command::atomic(self.policy, &bytes, 0o600)
    }
}

fn read<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !file.metadata()?.is_file() {
        return Err(Error::Refused(
            "Remote access settings are not a regular file.",
        ));
    }
    let mut bytes = Vec::new();
    file.take(16385).read_to_end(&mut bytes)?;
    if bytes.len() > 16384 {
        return Err(Error::Refused(
            "Remote access settings exceeded their limit.",
        ));
    }
    Ok(Some(serde_json::from_slice(&bytes)?))
}

fn sync_parent(path: &Path) -> Result<()> {
    let parent = path.parent().ok_or(Error::Refused("Invalid state path."))?;
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests;
