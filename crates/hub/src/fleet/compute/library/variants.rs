//! The variants of one repository (LIBM-002): one per GGUF quantisation, one for top-level safetensors.

use std::collections::BTreeMap;

use super::{
    model::Format,
    repository::{Listing, ModelConfig, Sibling},
};

/// A file outside LFS has no SHA-256 in the listing; the hub reads the file and computes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedFile {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub format: Format,
    pub quantisation: String,
    pub files: Vec<PlannedFile>,
}

impl Planned {
    /// The variant's tag (LIBM-006).
    #[must_use]
    pub fn tag(&self) -> String {
        self.quantisation.to_ascii_lowercase()
    }
}

/// Code files (`.py`) are never taken: a runtime must not run code from a repository.
const SIDE_FILES: [&str; 6] = [
    ".json",
    ".model",
    ".jinja",
    ".tiktoken",
    "merges.txt",
    "vocab.txt",
];

fn planned(file: &Sibling) -> PlannedFile {
    PlannedFile {
        path: file.rfilename.clone(),
        size_bytes: file
            .lfs
            .as_ref()
            .map(|lfs| lfs.size)
            .or(file.size)
            .unwrap_or(0),
        sha256: file.lfs.as_ref().map(|lfs| lfs.sha256.to_ascii_lowercase()),
    }
}

/// `UD` is the word of Unsloth's dynamic quantisations.
fn is_quantisation_word(word: &str) -> bool {
    let upper = word.to_ascii_uppercase();
    let numbered = |prefix: &str| {
        upper
            .strip_prefix(prefix)
            .is_some_and(|rest| rest.starts_with(|each: char| each.is_ascii_digit()))
    };
    numbered("Q")
        || numbered("IQ")
        || numbered("TQ")
        || numbered("MXFP")
        || ["F16", "BF16", "F32", "UD"].contains(&upper.as_str())
}

fn gguf_quantisation(path: &str) -> Option<String> {
    let name = path.rsplit('/').next()?;
    let name = name.get(..name.len().checked_sub(5)?)?;
    let name = strip_part(name);
    let words: Vec<&str> = name.split(['-', '.']).collect();
    let start = words
        .iter()
        .rposition(|word| !is_quantisation_word(word))
        .map_or(0, |last| last + 1);
    (start < words.len()).then(|| words[start..].join("-"))
}

/// The name of a GGUF file without `-00001-of-00003`.
fn strip_part(name: &str) -> &str {
    let bytes = name.as_bytes();
    let is_part = bytes.len() > 15
        && name[bytes.len() - 15..].starts_with('-')
        && name[bytes.len() - 9..].starts_with("-of-");
    if is_part {
        &name[..bytes.len() - 15]
    } else {
        name
    }
}

/// Vision projectors (`mmproj`) are left out (LIBM-010).
fn is_model_gguf(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".gguf")
        && !path
            .rsplit('/')
            .next()
            .unwrap_or(path)
            .starts_with("mmproj")
}

fn gguf_variants(listing: &Listing) -> Vec<Planned> {
    let mut grouped: BTreeMap<String, Vec<PlannedFile>> = BTreeMap::new();
    for file in listing
        .siblings
        .iter()
        .filter(|file| is_model_gguf(&file.rfilename))
    {
        if let Some(quantisation) = gguf_quantisation(&file.rfilename) {
            grouped.entry(quantisation).or_default().push(planned(file));
        }
    }
    grouped
        .into_iter()
        .map(|(quantisation, files)| Planned {
            format: Format::Gguf,
            quantisation,
            files,
        })
        .collect()
}

fn is_top(path: &str) -> bool {
    !path.contains('/')
}

fn safetensors_variant(listing: &Listing, config: Option<&ModelConfig>) -> Option<Planned> {
    let weights = listing
        .siblings
        .iter()
        .any(|file| is_top(&file.rfilename) && file.rfilename.ends_with(".safetensors"));
    if !weights {
        return None;
    }
    let files = listing
        .siblings
        .iter()
        .filter(|file| {
            let path = &file.rfilename;
            is_top(path)
                && (path.ends_with(".safetensors")
                    || SIDE_FILES.iter().any(|end| path.ends_with(end)))
        })
        .map(planned)
        .collect();
    let is_mlx = listing.library_name.as_deref() == Some("mlx")
        || listing.tags.iter().any(|tag| tag == "mlx");
    Some(Planned {
        format: if is_mlx {
            Format::Mlx
        } else {
            Format::Safetensors
        },
        quantisation: config
            .and_then(ModelConfig::quantisation)
            .unwrap_or_else(|| "default".to_owned()),
        files,
    })
}

#[must_use]
pub fn plan(listing: &Listing, config: Option<&ModelConfig>) -> Vec<Planned> {
    let mut found = gguf_variants(listing);
    found.extend(safetensors_variant(listing, config));
    for variant in &mut found {
        variant.files.sort_by(|a, b| a.path.cmp(&b.path));
    }
    found
}
