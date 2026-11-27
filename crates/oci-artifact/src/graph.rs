use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use super::{Blob, Error, Image, Platform, Result, Verified, invalid, provenance::Evidence};

pub(super) fn verify(
    index: &Value,
    blobs: &BTreeMap<String, Blob>,
    claimed: &str,
) -> Result<Verified> {
    let mut traversal = Traversal {
        blobs,
        claimed,
        selected: None,
        budget: 10_000,
        provenance: Evidence::default(),
    };
    traversal.index(index, 0)?;
    Ok(Verified {
        image: traversal.selected.ok_or(Error::Invalid)?,
        provenance: traversal.provenance,
    })
}

struct Traversal<'a> {
    blobs: &'a BTreeMap<String, Blob>,
    claimed: &'a str,
    selected: Option<Image>,
    budget: usize,
    provenance: Evidence,
}

impl Traversal<'_> {
    fn index(&mut self, value: &Value, depth: usize) -> Result<BTreeSet<Platform>> {
        if depth > 8 || value["schemaVersion"] != 2 {
            return invalid();
        }
        let manifests = value["manifests"].as_array().ok_or(Error::Invalid)?;
        if manifests.is_empty() || manifests.len() > 256 {
            return invalid();
        }
        let mut platforms = BTreeSet::new();
        for descriptor in manifests {
            platforms.extend(self.visit(descriptor, depth + 1)?);
        }
        self.provenance.inspect(value, self.blobs)?;
        Ok(platforms)
    }

    fn visit(&mut self, descriptor: &Value, depth: usize) -> Result<BTreeSet<Platform>> {
        if self.budget == 0 || depth > 8 {
            return invalid();
        }
        self.budget -= 1;
        let (digest, blob) = resolve(descriptor, self.blobs)?;
        let media_type = descriptor["mediaType"].as_str().ok_or(Error::Invalid)?;
        let body = blob.json.as_ref().ok_or(Error::Invalid)?;
        let value: Value = serde_json::from_slice(body)?;
        if value["schemaVersion"] != 2
            || value
                .get("mediaType")
                .is_some_and(|declared| declared != media_type)
        {
            return invalid();
        }
        let platforms = match media_type {
            "application/vnd.oci.image.index.v1+json"
            | "application/vnd.docker.distribution.manifest.list.v2+json" => {
                if is_attachment(descriptor, &value) {
                    return invalid();
                }
                self.index(&value, depth)?
            }
            "application/vnd.oci.image.manifest.v1+json"
            | "application/vnd.docker.distribution.manifest.v2+json" => {
                self.manifest(descriptor, &value)?
            }
            _ => return invalid(),
        };
        if platforms.len() > 256 {
            return invalid();
        }
        if digest == self.claimed && !platforms.is_empty() {
            let selected = Image {
                digest: digest.to_owned(),
                media_type: media_type.to_owned(),
                platforms: platforms.iter().cloned().collect(),
            };
            if self
                .selected
                .as_ref()
                .is_some_and(|existing| existing != &selected)
            {
                return invalid();
            }
            self.selected = Some(selected);
        }
        Ok(platforms)
    }

    fn manifest(&self, descriptor: &Value, value: &Value) -> Result<BTreeSet<Platform>> {
        let (_, config) = resolve(&value["config"], self.blobs)?;
        let layers = value["layers"].as_array().ok_or(Error::Invalid)?;
        if layers.len() > 10_000 {
            return invalid();
        }
        for layer in layers {
            resolve(layer, self.blobs)?;
        }
        if is_attachment(descriptor, value) {
            return Ok(BTreeSet::new());
        }
        if !layers.iter().all(filesystem_layer) {
            return invalid();
        }
        let config_type = value["config"]["mediaType"].as_str();
        if !matches!(
            config_type,
            Some(
                "application/vnd.oci.image.config.v1+json"
                    | "application/vnd.docker.container.image.v1+json"
            )
        ) {
            return invalid();
        }
        let bytes = config.json.as_ref().ok_or(Error::Invalid)?;
        let config: Value = serde_json::from_slice(bytes)?;
        let platform = image_platform(&config, descriptor.get("platform"), layers.len())?;
        Ok(BTreeSet::from([platform]))
    }
}

fn filesystem_layer(descriptor: &Value) -> bool {
    matches!(
        descriptor["mediaType"].as_str(),
        Some(
            "application/vnd.oci.image.layer.v1.tar"
                | "application/vnd.oci.image.layer.v1.tar+gzip"
                | "application/vnd.oci.image.layer.v1.tar+zstd"
                | "application/vnd.oci.image.layer.nondistributable.v1.tar"
                | "application/vnd.oci.image.layer.nondistributable.v1.tar+gzip"
                | "application/vnd.oci.image.layer.nondistributable.v1.tar+zstd"
                | "application/vnd.docker.image.rootfs.diff.tar"
                | "application/vnd.docker.image.rootfs.diff.tar.gzip"
                | "application/vnd.docker.image.rootfs.foreign.diff.tar.gzip"
        )
    )
}

fn is_attachment(descriptor: &Value, value: &Value) -> bool {
    descriptor["annotations"]
        .get("vnd.docker.reference.type")
        .is_some()
        || value.get("artifactType").is_some()
        || value.get("subject").is_some()
        || descriptor["platform"]["os"] == "unknown"
        || descriptor["platform"]["architecture"] == "unknown"
}

fn image_platform(config: &Value, descriptor: Option<&Value>, layers: usize) -> Result<Platform> {
    let root = &config["rootfs"];
    if root["type"] != "layers"
        || root["diff_ids"]
            .as_array()
            .is_none_or(|ids| ids.len() != layers || !ids.iter().all(sha256))
    {
        return invalid();
    }

    let mut platform = platform(config)?;
    if let Some(descriptor) = descriptor {
        let named = self::platform(descriptor)?;
        if named.os != platform.os
            || named.architecture != platform.architecture
            || named
                .variant
                .as_ref()
                .zip(platform.variant.as_ref())
                .is_some_and(|(left, right)| left != right)
        {
            return invalid();
        }
        platform.variant = named.variant.or(platform.variant);
    }
    Ok(platform)
}

fn sha256(value: &Value) -> bool {
    value
        .as_str()
        .and_then(|value| value.strip_prefix("sha256:"))
        .is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}

fn platform(value: &Value) -> Result<Platform> {
    let os = value["os"]
        .as_str()
        .filter(|value| platform_word(value))
        .ok_or(Error::Invalid)?;
    let architecture = value["architecture"]
        .as_str()
        .filter(|value| platform_word(value))
        .ok_or(Error::Invalid)?;
    let variant = value
        .get("variant")
        .map(|value| {
            value
                .as_str()
                .filter(|value| platform_word(value))
                .ok_or(Error::Invalid)
        })
        .transpose()?;
    Ok(Platform {
        os: os.to_owned(),
        architecture: architecture.to_owned(),
        variant: variant.map(str::to_owned),
    })
}

fn platform_word(word: &str) -> bool {
    !word.is_empty()
        && word != "unknown"
        && word.len() <= 32
        && word.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

pub(super) fn resolve<'a>(
    descriptor: &'a Value,
    blobs: &'a BTreeMap<String, Blob>,
) -> Result<(&'a str, &'a Blob)> {
    let digest = descriptor["digest"].as_str().ok_or(Error::Invalid)?;
    let blob = blobs.get(digest).ok_or(Error::Invalid)?;
    if descriptor["size"].as_u64() != Some(blob.size) || descriptor.get("urls").is_some() {
        return invalid();
    }
    Ok((digest, blob))
}
