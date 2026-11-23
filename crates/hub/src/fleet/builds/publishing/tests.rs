//! Tests of the publishing rules (BLD-060 to BLD-069, D-24, D-25, R-108, R-117).

mod feeds;
mod files;
mod holds;
mod parsers;
mod records;
mod signatures;
mod sigv4;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use minisign::KeyPair;

use super::updater_key::UpdaterKey;
use crate::fleet::builds::model::UploadedFile;

fn file(name: &str, bytes: &[u8]) -> UploadedFile {
    UploadedFile {
        name: name.to_owned(),
        sha256: super::sigv4::sha256_hex(bytes),
        size: i64::try_from(bytes.len()).unwrap(),
    }
}

/// A throwaway key as `tauri signer generate` writes it: the key file, the `plugins.updater.pubkey` text, and the pair.
fn tauri_key(password: Option<&str>) -> (String, String, KeyPair) {
    let pair = match password {
        Some(password) => KeyPair::generate_encrypted_keypair(Some(password.to_owned())).unwrap(),
        None => KeyPair::generate_unencrypted_keypair().unwrap(),
    };
    let file = STANDARD.encode(pair.sk.to_box(None).unwrap().into_string());
    let public = STANDARD.encode(pair.pk.to_box().unwrap().into_string());
    (file, public, pair)
}

fn updater_key() -> UpdaterKey {
    let (file, public, _) = tauri_key(None);
    UpdaterKey::from_parts(file.as_bytes(), b"", &public).unwrap()
}
