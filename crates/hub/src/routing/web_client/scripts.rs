//! The CSP hashes of the inline scripts in `index.html` (A-2).
//! The page is the hub's own build output, not operator input. A misread can only drop a hash
//! or add one of text the page holds; it never widens `script-src`.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest as _, Sha256};

const OPEN: &str = "<script";
const CLOSE: &str = "</script";

#[must_use]
pub fn inline_script_hashes(html: &str) -> Vec<String> {
    // ASCII lowercasing keeps every byte offset, so offsets in `lower` slice `html`.
    let lower = html.to_ascii_lowercase();
    let mut hashes = Vec::new();
    let mut from = 0;
    while let Some(found) = lower[from..].find(OPEN) {
        let tag = from + found + OPEN.len();
        let Some(&next) = lower.as_bytes().get(tag) else {
            break;
        };
        if !(next.is_ascii_whitespace() || next == b'>' || next == b'/') {
            from = tag; // `<scripts`, `<script-x`: another element.
            continue;
        }
        let Some(tag_len) = tag_end(&lower[tag..]) else {
            break;
        };
        let body = tag + tag_len + 1;
        let Some(body_len) = lower[body..].find(CLOSE) else {
            break;
        };
        if !attribute_names(&lower[tag..tag + tag_len]).contains(&"src") {
            hashes.push(source_hash(&html[body..body + body_len]));
        }
        from = body + body_len + CLOSE.len();
    }
    hashes
}

/// A browser hashes an inline script after its HTML parser turns each CRLF and lone CR into LF.
fn source_hash(script: &str) -> String {
    let parsed = script.replace("\r\n", "\n").replace('\r', "\n");
    format!(
        "'sha256-{}'",
        STANDARD.encode(Sha256::digest(parsed.as_bytes()))
    )
}

fn tag_end(tag: &str) -> Option<usize> {
    let mut quote = None;
    for (offset, byte) in tag.bytes().enumerate() {
        match (quote, byte) {
            (Some(open), _) if byte == open => quote = None,
            (None, b'"' | b'\'') => quote = Some(byte),
            (None, b'>') => return Some(offset),
            _ => {}
        }
    }
    None
}

fn attribute_names(attributes: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut rest = attributes;
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == '/');
        if rest.is_empty() {
            return names;
        }
        let end = rest
            .find(|c: char| c.is_ascii_whitespace() || c == '=' || c == '/')
            .unwrap_or(rest.len());
        names.push(&rest[..end]);
        rest = rest[end..].trim_start_matches(|c: char| c.is_ascii_whitespace());
        if let Some(value) = rest.strip_prefix('=') {
            rest = after_value(value.trim_start_matches(|c: char| c.is_ascii_whitespace()));
        }
    }
}

fn after_value(value: &str) -> &str {
    match value.chars().next() {
        Some(quote @ ('"' | '\'')) => value[1..]
            .find(quote)
            .map_or("", |close| &value[close + 2..]),
        _ => value
            .find(|c: char| c.is_ascii_whitespace())
            .map_or("", |end| &value[end..]),
    }
}
