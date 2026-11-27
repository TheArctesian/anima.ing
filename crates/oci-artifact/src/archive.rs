//! Read OCI layout data and hash each blob without extracting the archive.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
};

use super::{Blob, Error, JSON_LIMIT, LIMIT, Result, hexadecimal, invalid};

#[derive(Default)]
pub(super) struct Contents {
    names: BTreeSet<String>,
    pub(super) blobs: BTreeMap<String, Blob>,
    pub(super) index: Option<Value>,
    pub(super) layout: Option<Value>,
    retained: usize,
}

pub(super) fn read_archive<R: Read>(file: &mut R) -> Result<Contents> {
    let mut archive = tar::Archive::new(file);
    let mut contents = Contents::default();
    for item in archive.entries().map_err(|_| Error::Invalid)? {
        contents.read_entry(&mut item.map_err(|_| Error::Invalid)?)?;
    }
    Ok(contents)
}

impl Contents {
    fn read_entry<R: Read>(&mut self, item: &mut tar::Entry<'_, R>) -> Result<()> {
        if self.names.len() >= 50000 {
            return invalid();
        }
        let name = std::str::from_utf8(&item.path_bytes())
            .map_err(|_| Error::Invalid)?
            .to_owned();
        if !self.names.insert(name.clone()) {
            return invalid();
        }
        if item.header().entry_type().is_dir() {
            if !matches!(
                name.as_str(),
                "blobs/" | "blobs" | "blobs/sha256/" | "blobs/sha256"
            ) || item.size() != 0
            {
                return invalid();
            }
            return Ok(());
        }
        if !item.header().entry_type().is_file() || item.size() > LIMIT {
            return invalid();
        }
        if name == "index.json" || name == "oci-layout" {
            if item.size() > JSON_LIMIT as u64 {
                return invalid();
            }
            let mut data = Vec::new();
            item.read_to_end(&mut data)?;
            let value = serde_json::from_slice::<Value>(&data)?;
            if name == "index.json" {
                self.index = Some(value);
            } else {
                self.layout = Some(value);
            }
            return Ok(());
        }
        let Some(hex) = name.strip_prefix("blobs/sha256/") else {
            return invalid();
        };
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return invalid();
        }
        let blob = read_blob(item, hex)?;
        self.retained += blob.json.as_ref().map_or(0, Vec::len);
        if self.retained > 32 * JSON_LIMIT {
            return invalid();
        }
        self.blobs.insert(format!("sha256:{hex}"), blob);
        Ok(())
    }
}

fn read_blob<R: Read>(item: &mut tar::Entry<'_, R>, hex: &str) -> Result<Blob> {
    let size = item.size();
    let offset = item.raw_file_position();
    let mut buffer = vec![0_u8; 65536];
    let mut hash = Sha256::new();
    let mut read = 0;
    let mut data = (size <= JSON_LIMIT as u64).then(Vec::new);
    loop {
        let count = item.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        read += count as u64;
        hash.update(&buffer[..count]);
        if let Some(data) = &mut data {
            data.extend_from_slice(&buffer[..count]);
        }
    }
    if read != size || hexadecimal(&hash.finalize()) != hex {
        return invalid();
    }
    Ok(Blob {
        offset,
        size,
        json: data,
    })
}
