//! Commit refs and file pages on GitHub (PRJ-010).

const HEX: &[u8; 16] = b"0123456789ABCDEF";

#[must_use]
pub fn is_commit(git_ref: &str) -> bool {
    git_ref.len() == 40 && git_ref.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Percent-encodes, so a name with a space, `#`, `?` or `%` stays one part of the address.
fn encoded(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/') {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(char::from(HEX[usize::from(byte >> 4)]));
            out.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    out
}

#[must_use]
pub fn file(home: &str, git_ref: &str, path: &str) -> String {
    format!(
        "{}/blob/{}/{}",
        home.trim_end_matches('/'),
        encoded(git_ref),
        encoded(path)
    )
}
