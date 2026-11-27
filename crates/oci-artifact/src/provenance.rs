//! Project only digest evidence from statements bound to a reachable runnable image.
use super::Result;
use super::{Blob, invalid};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const PREDICATE: &str = "https://slsa.dev/provenance/v0.2";

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    bound_statements: BTreeSet<String>,
    base_image_digests: BTreeSet<String>,
}

impl Evidence {
    pub(super) fn inspect(&mut self, index: &Value, blobs: &BTreeMap<String, Blob>) -> Result<()> {
        let Some(manifests) = index["manifests"].as_array() else {
            return Ok(());
        };
        for descriptor in manifests.iter().filter(|value| is_attestation(value)) {
            let subject = &descriptor["annotations"]["vnd.docker.reference.digest"];
            if !manifests.iter().any(|value| {
                !is_attestation(value)
                    && matches!(
                        value["mediaType"].as_str(),
                        Some(
                            "application/vnd.oci.image.manifest.v1+json"
                                | "application/vnd.docker.distribution.manifest.v2+json"
                        )
                    )
                    && value["digest"] == *subject
                    && subject.is_string()
            }) {
                return invalid();
            }
            let manifest = document(descriptor, blobs)?;
            let Some(layers) = manifest["layers"].as_array() else {
                return invalid();
            };
            for layer in layers {
                if layer["annotations"]["in-toto.io/predicate-type"] != PREDICATE {
                    continue;
                }
                if layer["mediaType"] != "application/vnd.in-toto+json" {
                    return invalid();
                }
                let statement = document(layer, blobs)?;
                self.statement(&statement, subject)?;
                let Some(statement_digest) = layer["digest"].as_str() else {
                    return invalid();
                };
                self.bound_statements.insert(statement_digest.to_owned());
                if self.bound_statements.len() > 256 || self.base_image_digests.len() > 1024 {
                    return invalid();
                }
            }
        }
        Ok(())
    }

    fn statement(&mut self, statement: &Value, subject: &Value) -> Result<()> {
        let Some(subjects) = statement["subject"].as_array() else {
            return invalid();
        };
        if statement["_type"] != "https://in-toto.io/Statement/v0.1"
            || statement["predicateType"] != PREDICATE
            || !subjects.iter().any(|value| {
                digest(&value["digest"]["sha256"])
                    .is_some_and(|digest| subject.as_str() == Some(digest.as_str()))
            })
        {
            return invalid();
        }
        let materials = &statement["predicate"]["materials"];
        if materials.is_null() {
            return Ok(());
        }
        let Some(materials) = materials.as_array() else {
            return invalid();
        };
        if materials.len() > 1024 {
            return invalid();
        }
        for material in materials {
            if material["uri"]
                .as_str()
                .is_some_and(|uri| uri.starts_with("pkg:docker/"))
            {
                let Some(digest) = digest(&material["digest"]["sha256"]) else {
                    return invalid();
                };
                self.base_image_digests.insert(digest);
            }
        }
        Ok(())
    }
}

fn is_attestation(value: &Value) -> bool {
    value["annotations"]["vnd.docker.reference.type"] == "attestation-manifest"
}

fn document(descriptor: &Value, blobs: &BTreeMap<String, Blob>) -> Result<Value> {
    let Some(bytes) = descriptor["digest"]
        .as_str()
        .and_then(|digest| blobs.get(digest))
        .and_then(|blob| blob.json.as_ref())
    else {
        return invalid();
    };
    Ok(serde_json::from_slice(bytes)?)
}

fn digest(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        .map(|value| format!("sha256:{value}"))
}
