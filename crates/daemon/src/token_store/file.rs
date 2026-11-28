//! The Linux token store: `token` in the daemon's folder, mode 0600, replaced by rename.

use std::path::PathBuf;
use std::time::Duration;

use zeroize::Zeroizing;

use super::TokenStoreError;
use crate::config::home::Home;
use crate::config::private_file;

/// A file never waits for the person, so each call takes the Keychain store's deadline unused.
#[derive(Clone)]
pub struct Backend {
    home: Home,
    path: PathBuf,
}

impl Backend {
    pub fn open(home: &Home) -> Self {
        Self {
            home: home.clone(),
            path: home.token_path(),
        }
    }

    #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
    pub(super) fn is_protected(&self) -> bool {
        crate::linux_role::covers(self.home.dir())
    }

    pub fn read(&self, _deadline: Duration) -> Result<Option<Zeroizing<Vec<u8>>>, TokenStoreError> {
        self.home.ensure()?;
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        if self.is_protected() {
            return crate::linux_role::credential()
                .map(|record| record.map(|(bytes, _, _)| bytes))
                .map_err(|_| TokenStoreError::LinuxAuthority);
        }
        Ok(private_file::read(&self.path)?)
    }

    pub fn write(
        &self,
        bytes: &[u8],
        device: Option<uuid::Uuid>,
        _deadline: Duration,
    ) -> Result<(), TokenStoreError> {
        self.home.ensure()?;
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        if self.is_protected() {
            return crate::linux_role::store(bytes, device)
                .map_err(|_| TokenStoreError::LinuxAuthority);
        }
        let _ = device;
        private_file::write(self.home.dir(), "token", bytes)?;
        Ok(())
    }

    pub fn delete(&self, _deadline: Duration) -> Result<(), TokenStoreError> {
        #[cfg(all(target_os = "linux", target_pointer_width = "64"))]
        if self.is_protected() {
            return crate::linux_role::forget().map_err(|_| TokenStoreError::LinuxAuthority);
        }
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(private_file::PrivateFileError::Io(error).into()),
        }
    }
}
