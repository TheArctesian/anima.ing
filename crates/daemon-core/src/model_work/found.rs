//! Found models: the models that a different program keeps on the machine, used where they are
//! (`contracts/compute/model-work.md` Sections 2.3, 8 and 10.2; PULL-012, R-443, R-444). The
//! walk of the folders is I/O in the daemon; the names, the keys and the record are here.

pub mod huggingface;
pub mod ollama;
pub mod owner;
pub mod record;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::folders::Installation;
use super::machine::MachineSettings;
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

/// A folder of Section 10.2 and the tool that keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Root {
    pub tool: Tool,
    pub path: PathBuf,
}

const OLLAMA_SYSTEM: &str = "/usr/share/ollama/.ollama/models";

/// The fixed folders of Section 10.2 for this installation: below the home folder of the user of
/// the daemon, and the Ollama folder of the system on Linux. No environment variable of a tool is
/// read (Section 10.2).
#[must_use]
pub fn tool_folders(installation: Installation, home: Option<&Path>) -> Vec<Root> {
    let root = |tool, path: PathBuf| Root { tool, path };
    let mut list = Vec::new();
    if installation != Installation::LinuxSystem
        && let Some(home) = home.filter(|home| home.is_absolute())
    {
        list.extend([
            root(Tool::Ollama, home.join(".ollama/models")),
            root(Tool::Huggingface, home.join(".cache/huggingface/hub")),
            root(Tool::LmStudio, home.join(".lmstudio/models")),
            root(Tool::LmStudio, home.join(".cache/lm-studio/models")),
        ]);
    }
    if installation != Installation::MacUser {
        list.push(root(Tool::Ollama, PathBuf::from(OLLAMA_SYSTEM)));
    }
    list
}

/// The folders that a walk reads now: none while `findModels` is false, else the fixed folders
/// and each folder of `modelFolders`.
#[must_use]
pub fn roots(fixed: &[Root], settings: &MachineSettings) -> Vec<Root> {
    if !settings.find_models {
        return Vec::new();
    }
    let named = settings.model_folders.iter().map(|path| Root {
        tool: Tool::Folder,
        path: PathBuf::from(path),
    });
    fixed.iter().cloned().chain(named).collect()
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

/// Whether the GGUF file `name` is a projector (`mmproj*.gguf`), which is never a model alone.
#[must_use]
pub fn is_projector(name: &str) -> bool {
    name.get(..6)
        .is_some_and(|start| start.eq_ignore_ascii_case("mmproj"))
}

/// The GGUF models of one folder: each file alone, or the parts of one split model in the order
/// of their numbers, and the projector of the folder last when it has exactly one. A split model
/// with a missing part is not a model.
#[must_use]
pub fn gguf_models<'a>(names: &[&'a str]) -> Vec<Vec<&'a str>> {
    let (projectors, mut names): (Vec<&str>, Vec<&str>) = names
        .iter()
        .copied()
        .filter(|name| is_gguf(name))
        .partition(|name| is_projector(name));
    names.sort_unstable();
    let mut models = models_of(&names);
    if let [projector] = projectors.as_slice() {
        for model in &mut models {
            model.push(*projector);
        }
    }
    models
}

fn models_of<'a>(names: &[&'a str]) -> Vec<Vec<&'a str>> {
    let mut models: Vec<Vec<&str>> = Vec::new();
    for name in names {
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
