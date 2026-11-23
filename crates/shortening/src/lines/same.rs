// Parts of this file are from Headroom (github.com/headroomlabs-ai/headroom), Copyright 2025
// Headroom Contributors, licensed under the Apache License, Version 2.0.
// Changed by Anima.ing, 2026: one pass over the characters in place of three regular
// expressions; an address is replaced before a number, so `0x1f` is one address.
// Source: crates/headroom-core/src/transforms/log_compressor.rs (`normalize_for_dedupe`)

//! The shape of a log line: two lines of one shape say the same with different numbers.

/// `line` with each number, address and path after its first `:` or `=` replaced by a fixed
/// word. The text before that mark stays, so two different messages never get one shape.
#[must_use]
pub fn shape(line: &str) -> String {
    let split = line.find([':', '=']).unwrap_or(line.len());
    let (head, tail) = line.split_at(split);
    let mut shape = String::with_capacity(line.len());
    shape.push_str(head);
    let mut rest = tail;
    while let Some(first) = rest.chars().next() {
        let (word, used) = replaced(rest, first);
        shape.push_str(word.unwrap_or(&rest[..used]));
        rest = &rest[used..];
    }
    shape
}

/// The fixed word for what `rest` starts with, if it starts with an address, a number or a
/// path, and how many bytes that takes.
fn replaced(rest: &str, first: char) -> (Option<&'static str>, usize) {
    if let Some(hex) = rest.strip_prefix("0x") {
        let digits = run(hex, |each| each.is_ascii_hexdigit());
        if digits > 0 {
            return (Some("ADDR"), 2 + digits);
        }
    }
    if first.is_ascii_digit() {
        return (Some("N"), run(rest, |each| each.is_ascii_digit()));
    }
    if first == '/' {
        let path = &rest[..run(rest, |each| {
            each == '/' || each == '_' || each.is_alphanumeric()
        })];
        if let Some(end) = path.rfind('/').filter(|end| *end > 0) {
            return (Some("/PATH/"), end + 1);
        }
    }
    (None, first.len_utf8())
}

/// The bytes at the start of `text` whose characters all pass `fits`.
fn run(text: &str, fits: impl Fn(char) -> bool) -> usize {
    text.find(|each| !fits(each)).unwrap_or(text.len())
}
