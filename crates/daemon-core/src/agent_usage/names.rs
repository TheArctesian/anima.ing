//! The alphabets and lengths of the texts in an `agentUsage` member (K8 Section 4). No text in
//! it is free text from the device: each passes one of these rules or is not sent.

use super::OTHER_MODEL;

const MAX_WORD: usize = 32;

const MAX_NAME: usize = 64;

const MAX_REPOSITORY: usize = 200;

const MAX_SEGMENT: usize = 100;

/// `owner/name` of a forge and up to two group levels; a deeper path is more likely a host folder.
const SEGMENTS: std::ops::RangeInclusive<usize> = 2..=4;

/// A remote path that starts with one is a folder on the host, so it is not sent (AIS-018).
const ROOT_FOLDERS: [&str; 11] = [
    "users", "home", "root", "var", "srv", "opt", "mnt", "volumes", "tmp", "private", "data",
];

#[must_use]
pub fn is_word(text: &str) -> bool {
    sized(text, MAX_WORD)
        && text
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
}

#[must_use]
pub fn is_session_id(text: &str) -> bool {
    sized(text, MAX_NAME)
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

#[must_use]
pub fn is_model(text: &str) -> bool {
    sized(text, MAX_NAME)
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

#[must_use]
pub fn model_or_other(name: &str) -> &str {
    if is_model(name) { name } else { OTHER_MODEL }
}

#[must_use]
pub fn is_repository(text: &str) -> bool {
    let mut parts = text.split('/');
    let host = parts.next().unwrap_or_default();
    let host_fits = !host.is_empty()
        && host.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        });
    let segments: Vec<&str> = parts.collect();
    text.len() <= MAX_REPOSITORY
        && host_fits
        && SEGMENTS.contains(&segments.len())
        && segments.iter().all(|segment| is_segment(segment))
        && !segments.first().is_some_and(|first| {
            ROOT_FOLDERS
                .iter()
                .any(|root| first.eq_ignore_ascii_case(root))
        })
}

fn is_segment(text: &str) -> bool {
    sized(text, MAX_SEGMENT)
        && text != "."
        && text != ".."
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

const fn sized(text: &str, most: usize) -> bool {
    !text.is_empty() && text.len() <= most
}
