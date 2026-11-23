//! The models of Ollama (Section 10.2): a manifest below `manifests/` names its layers by
//! SHA-256, and each layer is a file `blobs/sha256-<hex>`.

use serde::Deserialize;

use crate::model_work::names::is_lower_hex;

/// The most bytes of a manifest that the daemon reads.
pub const MAX_MANIFEST: usize = 64 * 1024;
/// The levels of a manifest below `manifests/`: host, namespace, model, tag.
pub const LEVELS: usize = 4;

const MODEL: &str = "application/vnd.ollama.image.model";
const PROJECTOR: &str = "application/vnd.ollama.image.projector";
const DEFAULT_HOST: &str = "registry.ollama.ai";
const DEFAULT_NAMESPACE: &str = "library";

/// A layer of a manifest: the SHA-256 of its blob and its size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blob {
    pub sha256: String,
    pub size_bytes: u64,
}

impl Blob {
    /// The name of its file in `blobs/`.
    #[must_use]
    pub fn file_name(&self) -> String {
        format!("sha256-{}", self.sha256)
    }
}

/// The GGUF layers of one model: the model, and at most one projector (Section 2.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layers {
    pub model: Blob,
    pub projector: Option<Blob>,
}

#[derive(Deserialize)]
struct Manifest {
    layers: Vec<Layer>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Layer {
    media_type: String,
    digest: String,
    size: u64,
}

fn blob(layer: &Layer) -> Option<Blob> {
    let sha256 = layer.digest.strip_prefix("sha256:")?;
    (is_lower_hex(sha256, 64) && layer.size > 0).then(|| Blob {
        sha256: sha256.to_owned(),
        size_bytes: layer.size,
    })
}

/// The layers of a manifest; `None` for a manifest that does not read, or that has no model
/// layer, two of one type, or a layer of a different form.
#[must_use]
pub fn layers(manifest: &[u8]) -> Option<Layers> {
    if manifest.len() > MAX_MANIFEST {
        return None;
    }
    let manifest: Manifest = serde_json::from_slice(manifest).ok()?;
    let of = |kind: &str| -> Option<Vec<Blob>> {
        manifest
            .layers
            .iter()
            .filter(|layer| layer.media_type == kind)
            .map(blob)
            .collect()
    };
    let (mut models, mut projectors) = (of(MODEL)?, of(PROJECTOR)?);
    if models.len() != 1 || projectors.len() > 1 {
        return None;
    }
    Some(Layers {
        model: models.remove(0),
        projector: projectors.pop(),
    })
}

/// The name that Ollama shows for the manifest at `host/namespace/model/tag`.
#[must_use]
pub fn shown_name([host, namespace, model, tag]: [&str; LEVELS]) -> String {
    match (host == DEFAULT_HOST, namespace == DEFAULT_NAMESPACE) {
        (true, true) => format!("{model}:{tag}"),
        (true, false) => format!("{namespace}/{model}:{tag}"),
        (false, _) => format!("{host}/{namespace}/{model}:{tag}"),
    }
}
