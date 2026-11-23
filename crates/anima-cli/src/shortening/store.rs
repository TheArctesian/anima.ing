//! The originals of shortened answers (SHA-010 to SHA-014): one private file for each, in a
//! private folder of the user, with a limit of size and a limit of age. Nothing here talks to
//! a network. Only this file reads, writes or removes an original.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::private_file::{self, PrivateFileError};

/// The most that all originals hold together (SHA-012).
pub const MOST_BYTES: u64 = 64 * 1024 * 1024;
/// The oldest original that is kept (SHA-012).
pub const OLDEST: Duration = Duration::from_hours(24);
/// The characters of a handle (SHA-011).
pub const HANDLE_LENGTH: usize = 24;

const ENDING: &str = ".json";

#[derive(Debug, thiserror::Error)]
pub enum OriginalError {
    #[error("A handle is {HANDLE_LENGTH} characters, each 0-9 or a-f.")]
    NotAHandle,
    #[error(
        "No original has this handle: it was removed after 24 hours, or to make room, or it was never here."
    )]
    Gone,
    #[error("The original is larger than {} bytes.", private_file::LIMIT)]
    TooLarge,
    #[error("The file of the original could not be used: {0}")]
    File(#[from] PrivateFileError),
    #[error("The folder of the originals could not be read: {0}")]
    Folder(#[from] std::io::Error),
}

/// One original, as it was stored.
#[derive(Debug, Clone, PartialEq)]
pub struct Original {
    /// The tool that gave the answer.
    pub tool: String,
    /// The whole answer, after MCP-008.
    pub answer: Value,
}

/// One file of the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    pub handle: String,
    pub bytes: u64,
    pub written: SystemTime,
}

/// The folder of the originals.
#[derive(Debug, Clone)]
pub struct Store {
    dir: PathBuf,
}

/// Whether `text` has the form of a handle; each other text is refused before the disk is read.
#[must_use]
pub fn is_handle(text: &str) -> bool {
    text.len() == HANDLE_LENGTH
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// The handle of an original: the start of its SHA-256 (SHA-011).
#[must_use]
pub fn handle_of(original: &str) -> String {
    let hash = Sha256::digest(original.as_bytes());
    let mut hex = String::with_capacity(hash.len() * 2);
    for byte in hash {
        // Writing to a `String` cannot fail.
        let _ = write!(hex, "{byte:02x}");
    }
    hex.truncate(HANDLE_LENGTH);
    hex
}

impl Store {
    #[must_use]
    pub const fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Stores `answer`, a whole answer of `tool`, and gives its handle. Old originals, and
    /// the oldest when there is no room, are removed first.
    ///
    /// # Errors
    /// [`OriginalError`] when the original is too large or the file cannot be written.
    pub fn put(
        &self,
        tool: &str,
        answer: &Value,
        now: SystemTime,
    ) -> Result<String, OriginalError> {
        let body = json!({ "tool": tool, "answer": answer }).to_string();
        if body.len() > private_file::LIMIT {
            return Err(OriginalError::TooLarge);
        }
        self.make_room(body.len() as u64, now)?;
        let handle = handle_of(&answer.to_string());
        private_file::write(&self.dir, &format!("{handle}{ENDING}"), body.as_bytes())?;
        Ok(handle)
    }

    /// The original with this handle.
    ///
    /// # Errors
    /// [`OriginalError::NotAHandle`], [`OriginalError::Gone`], or the fault of the file.
    pub fn get(&self, handle: &str, now: SystemTime) -> Result<Original, OriginalError> {
        if !is_handle(handle) {
            return Err(OriginalError::NotAHandle);
        }
        let path = self.dir.join(format!("{handle}{ENDING}"));
        let written = fs::metadata(&path).and_then(|file| file.modified());
        if written.is_ok_and(|written| too_old(written, now)) {
            private_file::remove(&path)?;
            return Err(OriginalError::Gone);
        }
        let bytes = private_file::read(&path)?.ok_or(OriginalError::Gone)?;
        let mut stored: Value = serde_json::from_slice(&bytes).map_err(|_| OriginalError::Gone)?;
        let tool = stored["tool"].as_str().unwrap_or_default().to_owned();
        Ok(Original {
            tool,
            answer: stored["answer"].take(),
        })
    }

    /// The originals, the oldest first. A folder that is not there holds none.
    ///
    /// # Errors
    /// [`OriginalError::Folder`] when the folder cannot be read.
    pub fn list(&self) -> Result<Vec<Kept>, OriginalError> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        let mut kept = Vec::new();
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let handle = name.to_str().and_then(|name| name.strip_suffix(ENDING));
            let Some(handle) = handle.filter(|handle| is_handle(handle)) else {
                continue;
            };
            let file = entry.metadata()?;
            kept.push(Kept {
                handle: handle.to_owned(),
                bytes: file.len(),
                written: file.modified()?,
            });
        }
        kept.sort_by(|left, right| {
            (left.written, &left.handle).cmp(&(right.written, &right.handle))
        });
        Ok(kept)
    }

    /// Removes each original, and says how many.
    ///
    /// # Errors
    /// [`OriginalError`] when the folder cannot be read or a file cannot be removed.
    pub fn clear(&self) -> Result<usize, OriginalError> {
        let kept = self.list()?;
        for each in &kept {
            self.remove(each)?;
        }
        Ok(kept.len())
    }

    /// Removes the originals that are too old, then the oldest until `incoming` more bytes fit.
    fn make_room(&self, incoming: u64, now: SystemTime) -> Result<(), OriginalError> {
        let mut total = incoming;
        let mut fresh = Vec::new();
        for each in self.list()? {
            if too_old(each.written, now) {
                self.remove(&each)?;
            } else {
                total += each.bytes;
                fresh.push(each);
            }
        }
        for each in fresh {
            if total <= MOST_BYTES {
                break;
            }
            self.remove(&each)?;
            total -= each.bytes;
        }
        Ok(())
    }

    fn remove(&self, kept: &Kept) -> Result<(), OriginalError> {
        Ok(private_file::remove(
            &self.dir.join(format!("{}{ENDING}", kept.handle)),
        )?)
    }
}

fn too_old(written: SystemTime, now: SystemTime) -> bool {
    now.duration_since(written).is_ok_and(|age| age > OLDEST)
}

#[cfg(test)]
mod tests;
