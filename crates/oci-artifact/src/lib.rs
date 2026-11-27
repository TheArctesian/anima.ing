//! Verify an OCI archive and select its exact claimed runnable image without extraction.
mod archive;
mod graph;
mod provenance;
mod publication;
#[cfg(test)]
mod tests;

use std::{
    fmt::Write as _,
    io::{self, Read, Seek},
};

pub use provenance::Evidence;
pub use publication::{Part, Publication, publication};

use serde::Serialize;
use sha2::{Digest, Sha256};

const LIMIT: u64 = 8 * 1024 * 1024 * 1024;
const JSON_LIMIT: usize = 1024 * 1024;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the OCI archive failed integrity or image checks")]
    Invalid,
    #[error("the OCI archive could not be read")]
    Io(#[from] io::Error),
    #[error("the OCI archive contains invalid JSON")]
    Json(#[from] serde_json::Error),
}

struct Blob {
    offset: u64,
    size: u64,
    json: Option<Vec<u8>>,
}

/// The archive identity and the exact descriptor declared by the producer.
#[derive(Clone, Copy)]
pub struct Claim<'a> {
    pub bytes: u64,
    pub archive_digest: &'a str,
    pub manifest_digest: &'a str,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Platform {
    pub os: String,
    pub architecture: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Image {
    pub digest: String,
    pub media_type: String,
    /// Runtime platforms below the selected descriptor.
    pub platforms: Vec<Platform>,
}

pub struct Verified {
    pub image: Image,
    pub provenance: Evidence,
}

/// Verify a stable archive handle against its identity and rewind it on success.
/// Callers own the storage, file permissions and authority to consume the image.
///
/// # Errors
/// A byte, digest, layout or descriptor mismatch, or a read failure.
pub fn verify(archive: &mut (impl Read + Seek), claim: Claim<'_>) -> Result<Verified> {
    checked(archive, claim).map(|(verified, _)| verified)
}

fn checked(
    archive: &mut (impl Read + Seek),
    claim: Claim<'_>,
) -> Result<(Verified, archive::Contents)> {
    if !(1..=LIMIT).contains(&claim.bytes) {
        return invalid();
    }
    archive.rewind()?;
    check_archive(archive, claim)?;
    archive.rewind()?;
    let contents = archive::read_archive(&mut archive.take(claim.bytes))?;
    if contents
        .layout
        .as_ref()
        .and_then(|value| value["imageLayoutVersion"].as_str())
        != Some("1.0.0")
    {
        return invalid();
    }
    let index = contents.index.as_ref().ok_or(Error::Invalid)?;
    let verified = graph::verify(index, &contents.blobs, claim.manifest_digest)?;
    archive.rewind()?;
    Ok((verified, contents))
}

fn check_archive(archive: &mut impl Read, claim: Claim<'_>) -> Result<()> {
    let mut hash = Sha256::new();
    let mut buffer = vec![0_u8; 65536];
    let mut bytes = 0_u64;
    let mut input = archive.take(claim.bytes + 1);
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        hash.update(&buffer[..count]);
    }
    if bytes != claim.bytes
        || format!("sha256:{}", hexadecimal(&hash.finalize())) != claim.archive_digest
    {
        return invalid();
    }
    Ok(())
}

fn hexadecimal(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(encoded, "{byte:02x}");
    }
    encoded
}

fn invalid<T>() -> Result<T> {
    Err(Error::Invalid)
}
