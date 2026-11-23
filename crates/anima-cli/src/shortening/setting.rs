//! The switch of the shortening (SHA-017): one small private file that says `on` or `off`.

use std::path::Path;

use crate::private_file::{self, PrivateFileError};

const FILE: &str = "state";

/// Whether the MCP server shortens its answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    On,
    Off,
}

/// The state when nothing is saved. Off until the saving is measured on real answers
/// (OD-SHA3 of the proposal `shortened-answers.md`).
pub const DEFAULT: State = State::Off;

impl State {
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::On => "on",
            Self::Off => "off",
        }
    }
}

/// The saved state. A file that is missing, exposed or holds a different word gives the
/// default, and never `on` by a fault when the default is off.
#[must_use]
pub fn read(dir: &Path) -> State {
    let saved = private_file::read(&dir.join(FILE)).ok().flatten();
    match saved.as_deref().map(|bytes| bytes.trim_ascii()) {
        Some(b"on") => State::On,
        Some(b"off") => State::Off,
        _ => DEFAULT,
    }
}

/// Saves the state.
///
/// # Errors
/// [`PrivateFileError`] when the file cannot be written.
pub fn write(dir: &Path, state: State) -> Result<(), PrivateFileError> {
    private_file::write(dir, FILE, state.word().as_bytes())
}
