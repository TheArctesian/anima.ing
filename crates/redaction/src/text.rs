//! Secrets inside free text (LOG-007, D-3 *Values*).

use super::{
    REDACTED,
    names::{Secrecy, secrecy},
    provider_keys::mask_provider_keys,
};

const SECRET_MARKERS: [&str; 1] = ["PuTTY-User-Key-File-"];

const SECRET_MARKERS_ANY_CASE: [&str; 2] = ["otpauth://", "authorization:"];

const SCHEMES: [&str; 3] = ["bearer", "basic", "token"];

const CREDENTIAL_MIN_LEN: usize = 8;

/// `"[redacted]"` if `text` holds a private key, an `otpauth://` URI or an `Authorization:`
/// header; otherwise `text` with every URL password, secret `key=value` or `key: value`, scheme
/// credential, JWT and provider key masked. Each pass takes time linear in the length of `text`.
#[must_use]
pub fn redact_text(text: &str) -> String {
    if is_secret_text(text) {
        return REDACTED.to_owned();
    }
    let masked = mask_url_userinfo(text);
    // Schemes before keys: masking `authorization=Basic` as a key value leaves the credential after it.
    let masked = mask_after_words(&masked, &SCHEMES, scheme_credential);
    let masked = mask_key_values(&masked);
    let masked = mask_jwts(&masked);
    mask_provider_keys(&masked)
}

fn is_secret_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    (text.contains("-----BEGIN") && text.contains("PRIVATE KEY"))
        || SECRET_MARKERS.iter().any(|marker| text.contains(marker))
        || SECRET_MARKERS_ANY_CASE
            .iter()
            .any(|marker| lower.contains(marker))
}

fn is_credential_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=' | b'-' | b'_' | b'.')
}

fn looks_like_credential(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.len() >= CREDENTIAL_MIN_LEN
        && bytes.iter().copied().all(is_credential_byte)
        && bytes.iter().enumerate().any(|(index, byte)| {
            byte.is_ascii_digit()
                || matches!(byte, b'+' | b'/' | b'=' | b'_')
                || (index > 0 && byte.is_ascii_uppercase())
        })
}

pub(super) fn starts_word(text: &str, index: usize) -> bool {
    index == 0 || !text.as_bytes()[index - 1].is_ascii_alphanumeric()
}

/// `secret(rest)` gives the byte range to mask in the text after each of `words` that starts a word.
fn mask_after_words(
    text: &str,
    words: &[&str],
    secret: impl Fn(&str) -> Option<(usize, usize)>,
) -> String {
    let lower = text.to_ascii_lowercase();
    let mut masked = String::with_capacity(text.len());
    let (mut copied, mut index) = (0, 0);
    while index < text.len() {
        let found = words
            .iter()
            .find(|word| lower[index..].starts_with(**word) && starts_word(text, index));
        let range = found.and_then(|word| {
            let after = index + word.len();
            secret(&text[after..]).map(|(start, end)| (after + start, after + end))
        });
        match range {
            Some((start, end)) => {
                masked.push_str(&text[copied..start]);
                masked.push_str(REDACTED);
                (copied, index) = (end, end);
            }
            None => index += text[index..].chars().next().map_or(1, char::len_utf8),
        }
    }
    masked.push_str(&text[copied..]);
    masked
}

fn is_key_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
}

fn mask_key_values(text: &str) -> String {
    let mut masked = String::with_capacity(text.len());
    let (mut copied, mut index) = (0, 0);
    while let Some(offset) = text[index..].find(['=', ':']) {
        let separator = index + offset;
        index = separator + 1;
        let secret_key =
            key_before(text, separator).is_some_and(|key| secrecy(key) != Secrecy::Plain);
        if let Some((start, end)) = secret_key.then(|| value_after(text, index)).flatten() {
            masked.push_str(&text[copied..start]);
            masked.push_str(REDACTED);
            (copied, index) = (end, end);
        }
    }
    masked.push_str(&text[copied..]);
    masked
}

fn key_before(text: &str, separator: usize) -> Option<&str> {
    let bytes = text.as_bytes();
    let mut end = separator;
    while end > 0 && matches!(bytes[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    if end > 0 && matches!(bytes[end - 1], b'"' | b'\'') {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && is_key_byte(bytes[start - 1]) {
        start -= 1;
    }
    (start < end).then(|| &text[start..end])
}

fn value_after(text: &str, from: usize) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut start = from;
    while start < bytes.len() && matches!(bytes[start], b' ' | b'\t' | b'\r' | b'\n') {
        start += 1;
    }
    // A search fails only from the last quote of its kind, so the searches stay linear.
    if let Some(&quote @ (b'"' | b'\'')) = bytes.get(start) {
        start += 1;
        if let Some(close) = bytes[start..].iter().position(|byte| *byte == quote) {
            return (close > 0).then_some((start, start + close));
        }
    }
    if matches!(bytes.get(start), Some(b'{' | b'[')) {
        return None;
    }
    let len = text[start..]
        .find(|c: char| c.is_whitespace() || matches!(c, '&' | ';' | ',' | '}' | '"' | '\''))
        .unwrap_or(text.len() - start);
    let value = &text[start..start + len];
    let boolean = value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false");
    (len > 0 && !boolean).then_some((start, start + len))
}

fn scheme_credential(rest: &str) -> Option<(usize, usize)> {
    let gap = rest
        .bytes()
        .take_while(|byte| byte.is_ascii_whitespace() || *byte == b':')
        .count();
    if gap == 0 {
        return None;
    }
    let start = gap + usize::from(matches!(rest.as_bytes().get(gap), Some(b'"' | b'\'')));
    let len = rest.as_bytes()[start..]
        .iter()
        .take_while(|byte| is_credential_byte(**byte))
        .count();
    looks_like_credential(&rest[start..start + len]).then_some((start, start + len))
}

fn mask_jwts(text: &str) -> String {
    let mut masked = String::with_capacity(text.len());
    let (mut copied, mut scanned_to) = (0, 0);
    for (start, _) in text.match_indices("eyJ") {
        // Skipping an `eyJ` inside a scanned run keeps the pass linear (`"eyJ.".repeat(…)`).
        if start < scanned_to || !starts_word(text, start) {
            continue;
        }
        let len = text[start..]
            .bytes()
            .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            .count();
        scanned_to = start + len;
        let run = &text[start..start + len];
        let parts: Vec<&str> = run.split('.').collect();
        // `header.payload.` is an unsigned JWT; a fourth, empty part is a full stop after one.
        let jwt_len = match parts.as_slice() {
            [_, payload, _] if !payload.is_empty() => Some(run.len()),
            [_, payload, _, ""] if !payload.is_empty() => Some(run.len() - 1),
            _ => None,
        };
        if let Some(jwt_len) = jwt_len {
            masked.push_str(&text[copied..start]);
            masked.push_str(REDACTED);
            copied = start + jwt_len;
        }
    }
    masked.push_str(&text[copied..]);
    masked
}

fn mask_url_userinfo(text: &str) -> String {
    let mut masked = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(scheme_end) = rest.find("://") {
        let (before, after) = rest.split_at(scheme_end + 3);
        masked.push_str(before);
        rest = match userinfo_secret(after) {
            Some((start, end)) => {
                masked.push_str(&after[..start]);
                masked.push_str(REDACTED);
                &after[end..]
            }
            None => after,
        };
    }
    masked.push_str(rest);
    masked
}

/// Without an `@` in the authority, a non-port `:` may start an unencoded password (`u:pa/ss@h`).
/// That scan stops at whitespace or `://`, so the pass over all URLs stays linear.
fn userinfo_secret(url: &str) -> Option<(usize, usize)> {
    let authority_len = url
        .find(|c: char| matches!(c, '/' | '?' | '#') || c.is_whitespace())
        .unwrap_or(url.len());
    let authority = &url[..authority_len];
    if let Some(at) = authority.rfind('@') {
        let userinfo = &authority[..at];
        return match userinfo.find(':') {
            Some(colon) => Some((colon + 1, at)),
            None => looks_like_credential(userinfo).then_some((0, at)),
        };
    }
    let host_start = if authority.starts_with('[') {
        authority.find(']')?
    } else {
        0
    };
    let colon = host_start + authority[host_start..].find(':')?;
    let port = &authority[colon + 1..];
    if !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let tail = &url[authority_len..];
    let at = tail
        .char_indices()
        .take_while(|(index, c)| !c.is_whitespace() && !tail[*index..].starts_with("://"))
        .find(|(_, c)| *c == '@')?
        .0;
    Some((colon + 1, authority_len + at))
}
