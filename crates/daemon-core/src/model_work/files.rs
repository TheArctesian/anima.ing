//! The files that a copy may have (Section 2.2, SEC-C-008, SEC-C-009); any other file is refused before a byte is downloaded.

pub mod classes;
pub mod code;

use super::names::{Format, is_source_name};

pub const MAX_FILES: usize = 64;
pub const MAX_NAME: usize = 256;

/// Suffixes that hold code when a program reads them; refused in each format and folder.
const CODE: [&str; 8] = [".bin", ".pt", ".pth", ".pkl", ".py", ".npz", ".ckpt", ".h5"];

const SAFETENSORS_NAMES: [&str; 7] = [
    "config.json",
    "generation_config.json",
    "tokenizer.json",
    "tokenizer_config.json",
    "special_tokens_map.json",
    "chat_template.jinja",
    "preprocessor_config.json",
];

const SAFETENSORS_SUFFIXES: [&str; 3] = [".safetensors", ".safetensors.index.json", ".model"];

/// A path inside a repository (Section 2).
#[must_use]
pub fn is_file_path(name: &str) -> bool {
    name.len() <= MAX_NAME && !name.is_empty() && name.split('/').all(is_source_name)
}

/// `name` is already a valid path.
#[must_use]
pub fn permitted(format: Format, name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if CODE.iter().any(|suffix| lower.ends_with(suffix)) {
        return false;
    }
    let base = lower.rsplit('/').next().unwrap_or_default();
    match format {
        Format::Gguf => std::path::Path::new(base)
            .extension()
            .is_some_and(|suffix| suffix == "gguf"),
        Format::Safetensors | Format::Mlx => {
            SAFETENSORS_NAMES.contains(&base)
                || SAFETENSORS_SUFFIXES
                    .iter()
                    .any(|suffix| base.len() > suffix.len() && base.ends_with(suffix))
        }
    }
}
