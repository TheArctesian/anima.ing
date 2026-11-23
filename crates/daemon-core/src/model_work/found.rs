//! Found models: the models that a different program keeps on the machine, used where they are
//! (`contracts/compute/model-work.md` Sections 2.3, 8 and 10.2; PULL-012, R-443, R-444). The
//! walk of the folders is I/O in the daemon; the names, the keys and the record are here.

pub mod huggingface;
pub mod ollama;
pub mod record;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::names::{Word, is_source_name};

/// The most found models that one walk lists (Section 2.3).
pub const MAX_FOUND: usize = 256;
/// The most levels that one walk reads below a folder (Section 2.3).
pub const MAX_DEPTH: usize = 6;
/// The most entries that one walk reads in one folder (Section 2.3).
pub const MAX_ENTRIES: usize = 4096;
/// The most bytes of the name that the hub shows (Section 8).
pub const MAX_SHOWN: usize = 256;
/// The most levels below a folder of `modelFolders` at which a GGUF file is found (Section 10.2).
pub const FOLDER_DEPTH: usize = 3;

/// The program that keeps a found model (Section 8).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    Ollama,
    LmStudio,
    Huggingface,
    /// A folder of the setting `modelFolders`.
    Folder,
}

impl Word for Tool {
    const ALL: &'static [Self] = &[
        Self::Ollama,
        Self::LmStudio,
        Self::Huggingface,
        Self::Folder,
    ];
    fn as_str(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::LmStudio => "lm_studio",
            Self::Huggingface => "huggingface",
            Self::Folder => "folder",
        }
    }
}

/// The key of a found model: stable while the model stays where it is, and never a path that
/// the hub could send back (Section 2.3).
#[must_use]
pub fn key(tool: Tool, location: &str) -> String {
    let mut hash = Sha256::new();
    hash.update(tool.as_str());
    hash.update([0]);
    hash.update(location);
    hex(&hash.finalize())
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// A name that the report may hold: text from the device, so a fixed set of characters only.
#[must_use]
pub fn is_shown_name(name: &str) -> bool {
    (1..=MAX_SHOWN).contains(&name.len())
        && name.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':' | b'/' | b'+')
        })
}

/// One part of a location below a folder of Section 10.2: a flat name, never `.` or `..`.
#[must_use]
pub fn is_part(name: &str) -> bool {
    is_source_name(name)
}

/// A GGUF file name that a found model may have (Section 2.2).
#[must_use]
pub fn is_gguf(name: &str) -> bool {
    is_part(name)
        && std::path::Path::new(name)
            .extension()
            .is_some_and(|suffix| suffix.eq_ignore_ascii_case("gguf"))
}

/// The number of this part and the count of parts of a split GGUF (`name-00001-of-00003.gguf`).
fn split_part(name: &str) -> Option<(&str, u32, u32)> {
    let stem = name.get(..name.len().checked_sub(5)?)?;
    let (rest, count) = stem.rsplit_once("-of-")?;
    let (base, part) = rest.rsplit_once('-')?;
    let number = |text: &str| {
        (text.len() == 5 && text.bytes().all(|b| b.is_ascii_digit()))
            .then(|| text.parse::<u32>().ok())
            .flatten()
    };
    let (part, count) = (number(part)?, number(count)?);
    (part >= 1 && part <= count).then_some((base, part, count))
}

/// The GGUF models of one folder: each file alone, or the parts of one split model in the order
/// of their numbers. A split model with a missing part is not a model.
#[must_use]
pub fn gguf_models<'a>(names: &[&'a str]) -> Vec<Vec<&'a str>> {
    let mut models: Vec<Vec<&str>> = Vec::new();
    let mut names: Vec<&str> = names.iter().copied().filter(|name| is_gguf(name)).collect();
    names.sort_unstable();
    for name in &names {
        match split_part(name) {
            None => models.push(vec![name]),
            Some((base, 1, count)) => {
                let parts: Option<Vec<&str>> = (1..=count)
                    .map(|part| {
                        names
                            .iter()
                            .copied()
                            .find(|other| split_part(other) == Some((base, part, count)))
                    })
                    .collect();
                models.extend(parts);
            }
            Some(_) => {}
        }
    }
    models
}

/// Whether the daemon may read an entry: owned by its user, by root or by the owner of the
/// folder of Section 10.2 it is in, and writable by no group or other user (Section 2.3).
#[must_use]
pub const fn owner_permits(owner: u32, mode: u32, daemon: u32, folder_owner: u32) -> bool {
    let owner_holds = owner == daemon || owner == 0 || owner == folder_owner;
    owner_holds && mode & 0o022 == 0
}

/// A model that the walk found (Section 8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundModel {
    pub tool: Tool,
    /// The location below the folder: the base of the key, never sent.
    pub location: String,
    /// The name that the tool shows.
    pub name: String,
    /// The absolute folder of Section 10.2 that holds it.
    pub root: String,
    pub files: Vec<FoundFile>,
    /// Only GGUF is usable in this version (MW-P10).
    pub usable: bool,
}

/// A file of a found model, by its parts below the folder of Section 10.2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundFile {
    pub location: Vec<String>,
    pub size_bytes: u64,
    /// The SHA-256 that the tool names the file by, where it does (an Ollama or Hugging Face blob).
    pub named_sha256: Option<String>,
}

impl FoundModel {
    #[must_use]
    pub fn key(&self) -> String {
        key(self.tool, &self.location)
    }

    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.files.iter().map(|file| file.size_bytes).sum()
    }

    /// The entry of the list `found` of `model_report` (Section 8).
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "found": self.key(),
            "tool": self.tool.as_str(),
            "name": self.name,
            "format": if self.usable { "gguf" } else { "safetensors" },
            "files": self.files.len(),
            "bytes": self.bytes(),
            "usable": self.usable,
        })
    }
}
