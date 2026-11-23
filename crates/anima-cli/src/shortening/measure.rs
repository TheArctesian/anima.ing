//! The measure of the shortening (SHA-018, SHA-013); it holds no content of an answer.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::private_file;

const FILE: &str = "measure.json";

/// An estimate: no tokenizer of a model is used.
pub const BYTES_FOR_A_TOKEN: usize = 4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sizes {
    pub answers: u64,
    pub bytes_before: u64,
    /// Their bytes of JSON after, with the mark.
    pub bytes_after: u64,
}

impl Sizes {
    #[must_use]
    pub const fn tokens_before(&self) -> u64 {
        self.bytes_before / BYTES_FOR_A_TOKEN as u64
    }

    #[must_use]
    pub const fn tokens_after(&self) -> u64 {
        self.bytes_after / BYTES_FOR_A_TOKEN as u64
    }

    #[must_use]
    pub const fn saved_percent(&self) -> u64 {
        if self.bytes_before == 0 {
            return 0;
        }
        self.bytes_before.saturating_sub(self.bytes_after) * 100 / self.bytes_before
    }

    pub const fn add(&mut self, before: usize, after: usize) {
        self.answers += 1;
        self.bytes_before += before as u64;
        self.bytes_after += after as u64;
    }
}

pub type Measure = BTreeMap<String, Sizes>;

#[must_use]
pub fn read(dir: &Path) -> Measure {
    private_file::read(&dir.join(FILE))
        .ok()
        .flatten()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Two servers that write at the same time can lose one count.
pub fn record(dir: &Path, tool: &str, before: usize, after: usize) {
    let mut measure = read(dir);
    measure
        .entry(tool.to_owned())
        .or_default()
        .add(before, after);
    if let Ok(bytes) = serde_json::to_vec(&measure) {
        // The count is not worth a failed answer (SHA-006).
        let _lost = private_file::write(dir, FILE, &bytes);
    }
}
