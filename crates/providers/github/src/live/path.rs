//! No path can change the host, leave `/repos/{o}/{r}/contents/`, or add a query or a fragment;
//! the ref goes only as a query value (PRJ-033).

use source_exchange::Problem;

use crate::problem::problem;

const MOST_BYTES: usize = 1_024;
const MOST_SEGMENT_BYTES: usize = 255;
const MOST_SEGMENTS: usize = 40;
const MOST_REF_BYTES: usize = 255;

const HEX: &[u8; 16] = b"0123456789ABCDEF";

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error(
    "a path is relative, has no empty, dot or dot-dot segment, no control character or backslash, and is at most 1,024 bytes"
)]
pub(super) struct UnusablePath;

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("a ref has no control character, space, `..` or leading dash, and is at most 255 bytes")]
pub(super) struct UnusableRef;

/// Each byte outside `A-Za-z0-9-._~` is percent-encoded; the empty path is the repository root.
pub(super) fn encode(path: &str) -> Result<String, UnusablePath> {
    if path.is_empty() {
        return Ok(String::new());
    }
    if path.len() > MOST_BYTES {
        return Err(UnusablePath);
    }
    let mut out = String::with_capacity(path.len());
    for (index, segment) in path.split('/').enumerate() {
        let refused = segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.len() > MOST_SEGMENT_BYTES
            || index >= MOST_SEGMENTS
            || segment.chars().any(|c| c.is_control() || c == '\\');
        if refused {
            return Err(UnusablePath);
        }
        if index > 0 {
            out.push('/');
        }
        for byte in segment.bytes() {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
                out.push(char::from(byte));
            } else {
                out.push('%');
                out.push(char::from(HEX[usize::from(byte >> 4)]));
                out.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
        }
    }
    Ok(out)
}

fn check_ref(git_ref: &str) -> Result<(), UnusableRef> {
    let refused = git_ref.is_empty()
        || git_ref.len() > MOST_REF_BYTES
        || git_ref.starts_with('-')
        || git_ref.contains("..")
        || git_ref.chars().any(|c| c.is_control() || c.is_whitespace());
    if refused { Err(UnusableRef) } else { Ok(()) }
}

pub(super) fn contents_tail(path: &str) -> Result<String, Problem> {
    encode(path)
        .map(|encoded| format!("/contents/{encoded}"))
        .map_err(|UnusablePath| {
            problem(
                "path_unusable",
                "code",
                "the path cannot be used to read the repository",
            )
        })
}

pub(super) fn ref_value<'a>(git_ref: &'a str, subject: &str) -> Result<&'a str, Problem> {
    check_ref(git_ref).map(|()| git_ref).map_err(|UnusableRef| {
        problem(
            "ref_unusable",
            subject,
            "the ref cannot be used to read the repository",
        )
    })
}
