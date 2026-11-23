//! Reads a channel document, which holds the channel's secrets (CMD-025).

use std::path::Path;

use serde::de::DeserializeOwned;

use crate::failure::Failure;
use crate::input::{self, MAX_DOCUMENT};
use crate::invocation::Invocation;

/// # Errors
/// [`Failure::Input`] for an unreadable document; [`Failure::Usage`] for an invalid one.
pub fn read<T: DeserializeOwned>(invocation: &Invocation, from: &Path) -> Result<T, Failure> {
    let what = "the channel document";
    let bytes = input::read(invocation.prompter, from, what, MAX_DOCUMENT)?;
    input::json(&bytes, what, "anima notify channel create --help")
}
