//! A request path is read percent-decoded, so an encoded spelling gets the plain answer (K7).

/// A missing file here is 404, never `index.html`, so HTML is never served as a script (A-2).
const APP: &[u8] = b"/_app/";
const IMMUTABLE: &[u8] = b"/_app/immutable/";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wanted {
    /// A segment starts with `.` or holds a NUL: always 404.
    Hidden,
    /// `/` or `/index.html`: the `index.html` read when the web client was built.
    Index,
    /// A file under `/_app/`, or 404.
    Asset { immutable: bool },
    /// A file, or `index.html` for a client-side route.
    Page,
}

/// `path` is still percent-encoded. Segments split at `/` and `\`, so a backslash `..` is hidden;
/// a run of `/` counts as one, as it does when `ServeDir` finds the file.
#[must_use]
pub fn wanted(path: &str) -> Wanted {
    let mut decoded = percent_decoded(path.as_bytes());
    decoded.dedup_by(|later, earlier| *later == b'/' && *earlier == b'/');
    let hidden = decoded
        .split(|byte| matches!(byte, b'/' | b'\\'))
        .any(|segment| segment.first() == Some(&b'.') || segment.contains(&0));
    if hidden {
        Wanted::Hidden
    } else if decoded == b"/" || decoded == b"/index.html" {
        Wanted::Index
    } else if decoded.starts_with(APP) {
        Wanted::Asset {
            immutable: decoded.starts_with(IMMUTABLE),
        }
    } else {
        Wanted::Page
    }
}

/// One pass only, as a file server decodes; a `%` without two hex digits is kept.
fn percent_decoded(bytes: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut rest = bytes;
    while let Some((&first, tail)) = rest.split_first() {
        let escaped = match tail {
            [high, low, ..] if first == b'%' => hex(*high).zip(hex(*low)),
            _ => None,
        };
        if let Some((high, low)) = escaped {
            decoded.push(high << 4 | low);
            rest = &tail[2..];
        } else {
            decoded.push(first);
            rest = tail;
        }
    }
    decoded
}

const fn hex(digit: u8) -> Option<u8> {
    match digit {
        b'0'..=b'9' => Some(digit - b'0'),
        b'a'..=b'f' => Some(digit - b'a' + 10),
        b'A'..=b'F' => Some(digit - b'A' + 10),
        _ => None,
    }
}
