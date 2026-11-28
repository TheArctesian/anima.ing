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

    pub fn read(&self, _deadline: Duration) -> Result<Option<Zeroizing<Vec<u8>>>, TokenStoreError> {
        self.home.ensure()?;
        Ok(private_file::read(&self.path)?)
    }

    pub fn write(
        &self,
        bytes: &[u8],
        _device: Option<uuid::Uuid>,
        _deadline: Duration,
    ) -> Result<(), TokenStoreError> {
        self.home.ensure()?;
        private_file::write(self.home.dir(), "token", bytes)?;
        Ok(())
    }

    pub fn delete(&self, _deadline: Duration) -> Result<(), TokenStoreError> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(private_file::PrivateFileError::Io(error).into()),
        }
    }
}
