//! Text a reader never sees (ECMA-48 escapes, Unicode default-ignorables), removed so that
//! redaction reads a line as a terminal shows it (BLD-049).

use std::{iter::Peekable, str::Chars};

const ESC: char = '\u{1b}';

/// The format characters of Unicode 15 (general category Cf).
const FORMAT: [(char, char); 21] = [
    ('\u{ad}', '\u{ad}'),
    ('\u{600}', '\u{605}'),
    ('\u{61c}', '\u{61c}'),
    ('\u{6dd}', '\u{6dd}'),
    ('\u{70f}', '\u{70f}'),
    ('\u{890}', '\u{891}'),
    ('\u{8e2}', '\u{8e2}'),
    ('\u{180e}', '\u{180e}'),
    ('\u{200b}', '\u{200f}'),
    ('\u{202a}', '\u{202e}'),
    ('\u{2060}', '\u{2064}'),
    ('\u{2066}', '\u{206f}'),
    ('\u{feff}', '\u{feff}'),
    ('\u{fff9}', '\u{fffb}'),
    ('\u{110bd}', '\u{110bd}'),
    ('\u{110cd}', '\u{110cd}'),
    ('\u{13430}', '\u{1343f}'),
    ('\u{1bca0}', '\u{1bca3}'),
    ('\u{1d173}', '\u{1d17a}'),
    ('\u{e0001}', '\u{e0001}'),
    ('\u{e0020}', '\u{e007f}'),
];

/// The default-ignorable characters outside category Cf that show nothing.
const IGNORABLE: [(char, char); 6] = [
    ('\u{34f}', '\u{34f}'),
    ('\u{115f}', '\u{1160}'),
    ('\u{3164}', '\u{3164}'),
    ('\u{fe00}', '\u{fe0f}'),
    ('\u{ffa0}', '\u{ffa0}'),
    ('\u{e0100}', '\u{e01ef}'),
];

/// Bytes that end `ESC <byte>` as terminals read it; any other is shown, so `ESC` cannot hide a key's first letter.
fn ends_escape(c: char) -> bool {
    c.is_ascii_digit() || matches!(c, '=' | '>' | 'c' | 'D' | 'E' | 'H' | 'M')
}

pub(super) fn is_hidden(c: char) -> bool {
    FORMAT
        .iter()
        .chain(&IGNORABLE)
        .any(|&(first, last)| (first..=last).contains(&c))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Finals {
    Drop,
    Keep,
    /// Removed when real output ends sequences with it, else kept: an unusual final is more likely a letter.
    KeepRare,
}

impl Finals {
    fn keeps(self, c: char, usual: impl Fn(char) -> bool) -> bool {
        match self {
            Self::Drop => false,
            Self::Keep => true,
            Self::KeepRare => !usual(c),
        }
    }
}

/// `text` as a terminal shows it; an unended sequence takes the rest of the text, as on a terminal.
pub(super) fn visible(text: &str) -> String {
    read(text, Finals::Drop, false)
}

/// Each escape and hidden character becomes a space, so a key that [`visible`] would cut stays whole.
pub(super) fn spaced(text: &str) -> String {
    read(text, Finals::Keep, true)
}

/// Final bytes kept: `ESC [ pass ZWSP word` reads `password`.
pub(super) fn joined(text: &str) -> String {
    read(text, Finals::Keep, false)
}

/// As [`joined`], but usual final bytes go: `ESC [ pass ESC [0m word` reads `password`.
pub(super) fn joined_rare(text: &str) -> String {
    read(text, Finals::KeepRare, false)
}

fn read(text: &str, finals: Finals, spaced: bool) -> String {
    let gap = |shown: &mut String| {
        if spaced {
            shown.push(' ');
        }
    };
    let mut shown = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            ESC => {
                skip_escape(&mut chars, finals, &mut shown);
                gap(&mut shown);
            }
            // C1 CSI, and the C1 starts of OSC, DCS, SOS, PM and APC strings.
            '\u{9b}' => {
                skip_control_sequence(&mut chars, finals);
                gap(&mut shown);
            }
            intro @ ('\u{9d}' | '\u{90}' | '\u{98}' | '\u{9e}' | '\u{9f}') => {
                skip_string(
                    &mut chars,
                    finals.keeps(intro, |c| c == '\u{9d}'),
                    &mut shown,
                );
                gap(&mut shown);
            }
            c if is_hidden(c) => gap(&mut shown),
            c => shown.push(c),
        }
    }
    shown
}

fn skip_escape(chars: &mut Peekable<Chars<'_>>, finals: Finals, kept: &mut String) {
    match chars.peek().copied() {
        Some('[') => {
            chars.next();
            skip_control_sequence(chars, finals);
        }
        // A reading that keeps finals keeps the string as text: `P` and `X` may be a key's letters.
        Some(intro @ (']' | 'P' | 'X' | '^' | '_')) if !finals.keeps(intro, |c| c == ']') => {
            chars.next();
            skip_string(chars, false, kept);
        }
        Some('(' | ')' | '*' | '+') => {
            chars.next();
            chars.next_if(|&c| {
                ('\u{30}'..='\u{7e}').contains(&c) && !finals.keeps(c, |c| matches!(c, 'B' | '0'))
            });
        }
        _ => {
            chars.next_if(|&c| {
                ends_escape(c) && !finals.keeps(c, |c| c.is_ascii_digit() || matches!(c, '=' | '>'))
            });
        }
    }
}

fn skip_control_sequence(chars: &mut Peekable<Chars<'_>>, finals: Finals) {
    while chars
        .next_if(|c| ('\u{20}'..='\u{3f}').contains(c))
        .is_some()
    {}
    chars.next_if(|&c| ('\u{40}'..='\u{7e}').contains(&c) && !finals.keeps(c, |c| c == 'm'));
}

/// Any other `ESC` ends the string and stays, as terminals read it as the next sequence's start.
/// `keep_text` keeps the string's text, as a string can hide part of a key (`api ESC _ k ESC \ ey`).
fn skip_string(chars: &mut Peekable<Chars<'_>>, keep_text: bool, kept: &mut String) {
    while let Some(&c) = chars.peek() {
        match c {
            '\u{7}' | '\u{9c}' => {
                chars.next();
                return;
            }
            ESC => {
                let mut ahead = chars.clone();
                ahead.next();
                if ahead.peek() == Some(&'\\') {
                    chars.nth(1);
                }
                return;
            }
            c => {
                chars.next();
                if keep_text {
                    kept.push(c);
                }
            }
        }
    }
}
