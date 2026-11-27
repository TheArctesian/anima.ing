//! Only a successful new hub enrollment can establish this Keychain-protected binding.
//! Ordinary bearer renewal carries it forward. A legacy record has no evidence about its home.

use std::{os::unix::fs::MetadataExt as _, path::Path};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use uuid::Uuid;
use zeroize::{Zeroize as _, Zeroizing};

use crate::config::Config;

#[derive(Deserialize, Serialize, PartialEq, Eq)]
pub(super) struct Folder {
    path: [u8; 32],
    volume: u64,
    inode: u64,
}

impl Folder {
    pub(super) fn at(dir: &Path) -> Option<Self> {
        let dir = dir.canonicalize().ok()?;
        let stat = dir.metadata().ok()?;
        Some(Self {
            path: Sha256::digest(dir.as_os_str().as_encoded_bytes()).into(),
            volume: stat.dev(),
            inode: stat.ino(),
        })
    }
}

#[derive(Deserialize, Serialize)]
struct Authority {
    device: Uuid,
    home: Folder,
    seed: [u8; 32],
}

impl Drop for Authority {
    fn drop(&mut self) {
        self.seed.zeroize();
    }
}

#[derive(Deserialize)]
struct Record {
    hub: String,
    #[serde(rename = "holderEnrollment")]
    authority: Option<Authority>,
}

pub(super) fn at(bytes: &[u8], config: &Config, home: &Folder) -> Option<Zeroizing<[u8; 32]>> {
    let stored: Record = serde_json::from_slice(bytes).ok()?;
    let authority = stored.authority?;
    (stored.hub == config.hub.as_text()
        && authority.device == config.device
        && &authority.home == home)
        .then(|| Zeroizing::new(authority.seed))
}

pub(super) fn bound(bytes: &[u8]) -> Option<bool> {
    Some(
        serde_json::from_slice::<Record>(bytes)
            .ok()?
            .authority
            .is_some(),
    )
}

pub(super) fn owns(bytes: &[u8], home: &Folder) -> bool {
    serde_json::from_slice::<Record>(bytes)
        .is_ok_and(|record| record.authority.is_none_or(|a| &a.home == home))
}

pub(super) fn record(
    text: &str,
    home: Folder,
    device: Option<Uuid>,
    previous: Option<&[u8]>,
) -> Result<Zeroizing<String>, String> {
    let unreadable = "unreadable credential";
    let new: Record = serde_json::from_str(text).map_err(|_| unreadable)?;
    let authority = if let Some(device) = device {
        if device.is_nil() {
            return Err(unreadable.to_owned());
        }
        let mut authority = Authority {
            home,
            device,
            seed: [0; 32],
        };
        getrandom::fill(&mut authority.seed).map_err(|_| unreadable)?;
        Some(authority)
    } else if let Some(old) = previous {
        let old: Record = serde_json::from_slice(old).map_err(|_| unreadable)?;
        if old.authority.as_ref().is_some_and(|a| a.home != home) {
            return Err(unreadable.to_owned());
        }
        (old.hub == new.hub).then_some(old.authority).flatten()
    } else {
        None
    };
    let Some(authority) = authority else {
        return Ok(Zeroizing::new(text.to_owned()));
    };
    let mut bytes = Zeroizing::new(Vec::with_capacity(text.len() + 1024));
    bytes.extend_from_slice(text.strip_suffix('}').ok_or(unreadable)?.as_bytes());
    bytes.extend_from_slice(b",\"holderEnrollment\":");
    serde_json::to_writer(&mut *bytes, &authority).map_err(|_| unreadable)?;
    bytes.push(b'}');
    Ok(Zeroizing::new(
        String::from_utf8(std::mem::take(&mut *bytes)).map_err(|_| unreadable)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_original_enrollment_can_authorize_delivery_and_renew_its_holder() {
        let home = tempfile::tempdir().unwrap();
        let copied = tempfile::tempdir().unwrap();
        let folder = || Folder::at(home.path()).unwrap();
        let other = || Folder::at(copied.path()).unwrap();
        let mut config = Config::parse(br#"{"hub":"https://hub.example","device":"00000000-0000-0000-0000-000000000009","deviceName":"fixture"}"#).unwrap();
        let text = r#"{"hub":"https://hub.example","token":"synthetic"}"#;
        let legacy = record(text, folder(), None, Some(text.as_bytes())).unwrap();
        assert!(at(legacy.as_bytes(), &config, &folder()).is_none());
        let original = record(text, folder(), Some(config.device), None).unwrap();
        let key = at(original.as_bytes(), &config, &folder()).unwrap();
        assert!(at(original.as_bytes(), &config, &other()).is_none());
        assert!(record(text, other(), None, Some(original.as_bytes())).is_err());
        let renewed = record(text, folder(), None, Some(original.as_bytes())).unwrap();
        assert_eq!(*at(renewed.as_bytes(), &config, &folder()).unwrap(), *key);
        let fresh = record(text, folder(), Some(config.device), None).unwrap();
        assert_ne!(*at(fresh.as_bytes(), &config, &folder()).unwrap(), *key);
        config.device = Uuid::from_u128(10);
        assert!(at(renewed.as_bytes(), &config, &folder()).is_none());
    }
}
