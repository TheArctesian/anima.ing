//! BLD-045, BLD-046, BLD-061, BLD-065, UPD-010. `SHA256SUMS` holds the digests the build jobs
//! reported, so a file another step changed is refused.
//! The signature is checked with the public key the release commit names, never a key the
//! files brought.

use bytes::Bytes;
use serde_json::{Map, Value, json};
use updater_signature::{Prehash, PublicKey, SignatureCheck, Signed};

use super::{Built, BuiltError, refused};
use crate::fleet::builds::{
    model::UploadedFile,
    publishing::{
        feed::{self, Expected, SUMS},
        files, sigv4,
        updater_key::UpdaterKey,
    },
};

pub struct CheckedBuilt {
    /// Each file but the `.sig`.
    pub(super) files: Vec<UploadedFile>,
    pub(super) updater: String,
    pub(super) signature: String,
    /// The feed without the signature.
    pub(super) feed: String,
}

impl CheckedBuilt {
    #[must_use]
    pub fn updater(&self) -> &str {
        &self.updater
    }
}

fn measured(built: &Built) -> Result<(Vec<UploadedFile>, String), BuiltError> {
    let mut files: Vec<UploadedFile> = built
        .files
        .iter()
        .map(|file| UploadedFile {
            name: file.name.clone(),
            sha256: sigv4::sha256_hex(&file.bytes),
            size: i64::try_from(file.bytes.len()).unwrap_or(i64::MAX),
        })
        .collect();
    let updater = feed::updater_name(built.target, &files)
        .ok_or_else(|| {
            refused(format!(
                "A {} release needs one updater file.",
                built.target
            ))
        })?
        .to_owned();
    let signature = feed::signature_name(&updater);
    files.retain(|file| file.name != signature);
    Ok((files, updater))
}

pub(super) fn bytes_of<'a>(built: &'a Built, name: &str) -> Option<&'a Bytes> {
    let file = built.files.iter().find(|file| file.name == name)?;
    Some(&file.bytes)
}

/// Checks the signature with the updater's own verifier before it returns it (BLD-045, BLD-046).
/// `signed_at` is the trusted comment's time, so the same commit time gives the same text.
///
/// # Errors
/// `Refused` when `built` has no single updater file or the signature does not hold.
pub fn sign_built(
    key: &UpdaterKey,
    built: &Built,
    signed_at: u64,
) -> Result<(String, String), BuiltError> {
    let (_, updater) = measured(built)?;
    let version = built.version.to_string();
    let unsigned = || refused(format!("{updater} cannot be signed for {version}."));
    let bytes = bytes_of(built, &updater).ok_or_else(unsigned)?;
    let mut prehash = Prehash::new();
    prehash.update(bytes);
    let signed = Signed {
        timestamp: signed_at,
        file_name: &updater,
        version: &version,
    };
    let text = key
        .sign(&prehash.finish(), &signed)
        .map_err(|_| unsigned())?;
    let check = key.check(&text).map_err(|_| unsigned())?;
    let mut stream = check.stream().map_err(|_| unsigned())?;
    stream.update(bytes);
    stream.finish(&version).map_err(|_| unsigned())?;
    Ok((feed::signature_name(&updater), text))
}

fn unsigned_feed(built: &Built, public_base: &str, updater: &str) -> String {
    let url = format!("{public_base}/{}/{}/{updater}", built.target, built.version);
    let platforms: Map<String, Value> = feed::platform_keys(built.target)
        .iter()
        .map(|key| ((*key).to_owned(), json!({ "url": url })))
        .collect();
    json!({ "version": built.version.to_string(), "platforms": platforms }).to_string()
}

/// `public_key` is the `plugins.updater.pubkey` text (UPD-002).
fn verifies(public_key: &str, signature: &str, (bytes, version): (&[u8], &str)) -> bool {
    let Ok(public) = PublicKey::from_key_file(public_key) else {
        return false;
    };
    let Ok(check) = SignatureCheck::new(&public, signature) else {
        return false;
    };
    let Ok(mut stream) = check.stream() else {
        return false;
    };
    stream.update(bytes);
    stream.finish(version).is_ok()
}

/// # Errors
/// `Refused` with the first check that failed.
pub fn check_built(
    built: &Built,
    public_key: &str,
    public_base: &str,
) -> Result<CheckedBuilt, BuiltError> {
    let (files, updater) = measured(built)?;
    let signature_name = feed::signature_name(&updater);
    for file in &built.files {
        let known = file.name == signature_name
            || files::content_type(built.target, built.version, &file.name).is_some();
        if !known {
            return Err(refused(format!(
                "{} is not a file of a {} release of {}.",
                file.name, built.target, built.version
            )));
        }
    }
    files::stable_copy(built.target, &files).map_err(refused)?;
    let sums = bytes_of(built, SUMS).map_or(&[][..], |bytes| &bytes[..]);
    files::check_sums(sums, &files).map_err(refused)?;
    let unverified = || {
        refused(format!(
            "The signature of {updater} does not verify for {} with the updater public key.",
            built.version
        ))
    };
    let signature = bytes_of(built, &signature_name)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .ok_or_else(unverified)?;
    let updater_bytes = bytes_of(built, &updater).ok_or_else(unverified)?;
    let version = built.version.to_string();
    if !verifies(public_key, signature, (updater_bytes, &version)) {
        return Err(unverified());
    }
    let feed = unsigned_feed(built, public_base, &updater);
    let expected = Expected {
        target: built.target,
        version: built.version,
        public_base,
        files: &files,
        floor: None,
    };
    feed::check(&feed, &expected).map_err(refused)?;
    Ok(CheckedBuilt {
        files,
        updater,
        signature: signature.to_owned(),
        feed,
    })
}
