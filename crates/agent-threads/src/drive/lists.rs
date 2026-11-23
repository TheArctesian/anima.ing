//! The lists that a program gives: its models (TH104, TH117), its commands and its skills
//! (TH117). Their names and words come from the files of the program, of the person and of a
//! repository: untrusted text, cut here to one plain line.

use serde_json::Value;

pub mod models;

/// The most characters of one line of words: a name for the person or a description.
pub const LINE: usize = 200;
/// The most bytes of one key: a speed, a program of access, a command or a skill.
pub const KEY: usize = 64;

/// `value` as one line of at most [`LINE`] characters, with no control character and no mark
/// that changes the direction of the text; `None` when it is not a string or is blank.
pub fn line(value: &Value) -> Option<String> {
    let text = value.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    let shown = crate::project::cleaned(text, LINE - 1);
    Some(shown.trim().to_owned())
}

/// `value` as a key of at most [`KEY`] bytes of letters, digits, `_`, `-` and the bytes of
/// `more`, not starting with `-`; `None` for any other value: a key is never changed to fit.
pub fn key(value: &Value, more: &[u8]) -> Option<String> {
    let text = value.as_str()?;
    let fits = !text.is_empty()
        && text.len() <= KEY
        && !text.starts_with('-')
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-' || more.contains(&b));
    fits.then(|| text.to_owned())
}

/// At most `most` values of the array `value`; none when it is not an array.
pub fn list(value: &Value, most: usize) -> impl Iterator<Item = &Value> + Clone {
    value
        .as_array()
        .map_or(&[][..], Vec::as_slice)
        .iter()
        .take(most)
}
