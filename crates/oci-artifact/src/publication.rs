//! Project the selected image and its dependencies into verified archive ranges.
use std::{
    collections::BTreeMap,
    io::{Read, Seek},
};

use serde_json::Value;

use super::{Blob, Claim, Error, Image, Result, checked, graph::resolve, invalid};

/// An object to copy byte for byte from the verified archive handle.
pub struct Part {
    pub digest: String,
    pub offset: u64,
    pub size: u64,
    /// Present for a manifest or index. Configurations and layers are registry blobs.
    pub media_type: Option<String>,
}

pub struct Publication {
    pub image: Image,
    /// Dependencies precede their manifests. The selected descriptor is last.
    pub parts: Vec<Part>,
}

/// Verify an archive and return the reachable parts in publication order.
///
/// # Errors
/// Archive integrity failure, an invalid publication graph, or a read failure.
pub fn publication(archive: &mut (impl Read + Seek), claim: Claim<'_>) -> Result<Publication> {
    let (verified, contents) = checked(archive, claim)?;
    let image = verified.image;
    let parts = Builder {
        blobs: &contents.blobs,
        included: BTreeMap::new(),
        parts: Vec::new(),
        archive_bytes: claim.bytes,
    }
    .parts_of(&image)?;
    Ok(Publication { image, parts })
}

struct Builder<'a> {
    blobs: &'a BTreeMap<String, Blob>,
    included: BTreeMap<&'a str, Option<String>>,
    parts: Vec<Part>,
    archive_bytes: u64,
}

impl<'a> Builder<'a> {
    fn parts_of(mut self, image: &Image) -> Result<Vec<Part>> {
        let (digest, blob) = self
            .blobs
            .get_key_value(&image.digest)
            .ok_or(Error::Invalid)?;
        self.manifest((digest, blob), &image.media_type, 0)?;
        Ok(self.parts)
    }

    fn manifest(
        &mut self,
        (digest, blob): (&'a str, &Blob),
        media: &str,
        depth: usize,
    ) -> Result<()> {
        if depth > 8 {
            return invalid();
        }
        if let Some(seen) = self.included.get(digest) {
            return (seen.as_deref() == Some(media))
                .then_some(())
                .ok_or(Error::Invalid);
        }
        let bytes = blob.json.as_ref().ok_or(Error::Invalid)?;
        let value: Value = serde_json::from_slice(bytes)?;
        if value["schemaVersion"] != 2
            // Publication does not maintain the referrers tag on older registries.
            || value.get("subject").is_some()
            || value
                .get("mediaType")
                .is_some_and(|declared| declared != media)
        {
            return invalid();
        }
        match media {
            "application/vnd.oci.image.index.v1+json"
            | "application/vnd.docker.distribution.manifest.list.v2+json" => {
                for descriptor in value["manifests"].as_array().ok_or(Error::Invalid)? {
                    self.descriptor(descriptor, depth + 1)?;
                }
            }
            "application/vnd.oci.image.manifest.v1+json"
            | "application/vnd.docker.distribution.manifest.v2+json" => {
                self.blob(&value["config"])?;
                for layer in value["layers"].as_array().ok_or(Error::Invalid)? {
                    self.blob(layer)?;
                }
            }
            _ => return invalid(),
        }
        self.add(digest, blob, Some(media.to_owned()))
    }

    fn descriptor(&mut self, descriptor: &Value, depth: usize) -> Result<()> {
        let (digest, blob) = self.lookup(descriptor)?;
        self.manifest(
            (digest, blob),
            descriptor["mediaType"].as_str().ok_or(Error::Invalid)?,
            depth,
        )
    }

    fn blob(&mut self, descriptor: &Value) -> Result<()> {
        let (digest, blob) = self.lookup(descriptor)?;
        self.add(digest, blob, None)
    }

    fn lookup(&self, descriptor: &Value) -> Result<(&'a str, &'a Blob)> {
        let (digest, _) = resolve(descriptor, self.blobs)?;
        self.blobs
            .get_key_value(digest)
            .map(|(name, blob)| (name.as_str(), blob))
            .ok_or(Error::Invalid)
    }

    fn add(&mut self, digest: &'a str, blob: &Blob, media_type: Option<String>) -> Result<()> {
        if let Some(seen) = self.included.get(digest) {
            return (seen == &media_type).then_some(()).ok_or(Error::Invalid);
        }
        if blob
            .offset
            .checked_add(blob.size)
            .is_none_or(|end| end > self.archive_bytes)
        {
            return invalid();
        }
        self.included.insert(digest, media_type.clone());
        self.parts.push(Part {
            digest: digest.to_owned(),
            offset: blob.offset,
            size: blob.size,
            media_type,
        });
        Ok(())
    }
}
