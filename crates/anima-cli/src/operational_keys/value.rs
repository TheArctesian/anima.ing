//! The value of a key comes from a prompt with no echo, standard input or a 0600 file, never an
//! argument, and never from a terminal that echoes it (K-E5 Section 2.2, CMD-025).

use std::path::Path;

use zeroize::Zeroizing;

use crate::devices::credentials::input::password;
use crate::failure::Failure;
use crate::prompt::Prompter;

const WHAT: &str = "The value of the key";

/// With no file, a person at a terminal is asked with echo off; otherwise standard input is
/// read, and a terminal under `--no-input` is refused.
///
/// # Errors
/// [`Failure::Usage`] when it cannot ask; [`Failure::Input`] for an empty or unusable value.
pub fn read(prompter: Prompter, from_file: Option<&Path>) -> Result<Zeroizing<String>, Failure> {
    match from_file {
        Some(path) if path != Path::new("-") || !prompter.interactive() => {
            password(prompter, WHAT, Some(path))
        }
        _ if prompter.interactive() => password(prompter, WHAT, None),
        _ => password(prompter, WHAT, Some(Path::new("-"))),
    }
}
