//! Open the Linux build output safely and verify its declared OCI image.
use std::{fs, os::unix::fs::OpenOptionsExt as _, path::Path};

use oci_artifact::{Claim, Evidence};
use serde_json::Value;

use super::super::{Error, Result};

pub(super) struct Verified {
    pub digest: String,
    pub bytes: u64,
    pub manifest: String,
    pub provenance: Evidence,
}

pub(super) fn verify(path: &Path, claimed: &Value) -> Result<Verified> {
    open_verified(path, claimed).map(|(verified, _)| verified)
}

pub(super) fn open_verified(path: &Path, claimed: &Value) -> Result<(Verified, fs::File)> {
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)?;
    let claim = claim(claimed)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() != claim.bytes {
        return Err(invalid());
    }
    let verified = oci_artifact::verify(&mut file, claim).map_err(|_| invalid())?;
    Ok((
        Verified {
            digest: claim.archive_digest.to_owned(),
            bytes: claim.bytes,
            manifest: verified.image.digest,
            provenance: verified.provenance,
        },
        file,
    ))
}

fn claim(value: &Value) -> Result<Claim<'_>> {
    Ok(Claim {
        bytes: value["bytes"].as_u64().ok_or_else(invalid)?,
        archive_digest: value["archiveDigest"].as_str().ok_or_else(invalid)?,
        manifest_digest: value["manifestDigest"].as_str().ok_or_else(invalid)?,
    })
}

fn invalid() -> Error {
    Error::Refused("The OCI artifact failed independent integrity checks.")
}
