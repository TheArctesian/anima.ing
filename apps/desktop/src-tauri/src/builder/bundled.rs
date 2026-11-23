//! The upload sends only what this bundle step wrote, and only while its bytes are unchanged (D-13,
//! D-22). The lock is held only for short synchronous sections.

use std::{
    path::PathBuf,
    sync::{Mutex, PoisonError},
};

use super::job::{Commit, Target, Version};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    pub commit: Commit,
    pub target: Target,
    pub version: Version,
    pub path: PathBuf,
    /// SHA-256 of the file as the bundle step left it.
    pub sha256: [u8; 32],
}

#[derive(Debug, Default)]
pub struct Bundled {
    last: Mutex<Option<Bundle>>,
}

impl Bundled {
    fn with_last<T>(&self, change: impl FnOnce(&mut Option<Bundle>) -> T) -> T {
        change(&mut self.last.lock().unwrap_or_else(PoisonError::into_inner))
    }

    pub fn record(&self, bundle: Bundle) {
        self.with_last(|last| *last = Some(bundle));
    }

    pub fn forget(&self) {
        self.with_last(|last| *last = None);
    }

    pub fn of(&self, target: Target, version: &Version) -> Option<Bundle> {
        self.with_last(|last| {
            last.as_ref()
                .filter(|bundle| bundle.target == target && &bundle.version == version)
                .cloned()
        })
    }
}
