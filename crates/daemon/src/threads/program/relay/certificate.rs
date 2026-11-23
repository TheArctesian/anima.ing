//! Only the public CA is a file. Its lifetime belongs to the holder listener.

use std::io::{self, Write as _};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::PathBuf;

pub(super) struct Certificate(tempfile::TempDir);

impl Certificate {
    pub fn publish(pem: &str) -> io::Result<Self> {
        let folder = tempfile::Builder::new()
            .prefix("anima-relay-ca-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir()?;
        let certificate = Self(folder);
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(certificate.path())?
            .write_all(pem.as_bytes())?;
        Ok(certificate)
    }

    pub fn path(&self) -> PathBuf {
        self.0.path().join("ca.pem")
    }
}
