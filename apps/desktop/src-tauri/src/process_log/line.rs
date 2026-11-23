use std::fmt::Arguments;

use tauri_plugin_log::{TimezoneStrategy, log::Record};

pub(super) struct Stamp {
    pub date: (i32, u8, u8),
    pub time: (u8, u8, u8),
}

impl Stamp {
    pub(super) fn now() -> Self {
        let now = TimezoneStrategy::UseUtc.get_now();
        Self {
            date: (now.year(), u8::from(now.month()), now.day()),
            time: (now.hour(), now.minute(), now.second()),
        }
    }
}

/// `message` is the record's text as the logger hands it on (fern's rule: use it, not `record.args()`).
/// The web view chooses its own target and message (`log:allow-log`), so escaping keeps one record one line and unforgeable.
pub(super) fn line_of(stamp: &Stamp, record: &Record, message: &Arguments) -> String {
    let (year, month, day) = stamp.date;
    let (hour, minute, second) = stamp.time;
    let level = record.level();
    let mut line =
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z [{level}] [");
    push_escaped(&mut line, record.target());
    line.push_str("] ");
    push_escaped(&mut line, &message.to_string());
    line
}

fn is_unsafe_in_a_line(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{2028}' | '\u{2029}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
        )
}

fn push_escaped(line: &mut String, text: &str) {
    for c in text.chars() {
        if is_unsafe_in_a_line(c) {
            line.extend(c.escape_default());
        } else {
            line.push(c);
        }
    }
}
