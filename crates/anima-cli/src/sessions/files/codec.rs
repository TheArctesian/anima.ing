//! The files tunnel's text messages (K4 Section 9).
//! A hub message outside the closed set fails closed: a misread one could save the wrong bytes.
//! Entry names are device text: callers print them only through the table or JSON, which escape them.

use serde::{Deserialize, Serialize};

/// The longest path the hub takes, in bytes (K4 Section 9).
pub const MAX_PATH: usize = 4096;
/// The longest client text message (K4 Section 9).
pub const MAX_TEXT: usize = 4096;
/// The most entries one `entries` message may hold (K4 Section 9).
pub const MAX_ENTRIES: usize = 10_000;
/// The upload frame size; the hub takes up to 64 KiB (K4 Section 9).
pub const UPLOAD_FRAME: usize = 32 * 1024;
pub const PING: &str = r#"{"type":"ping"}"#;

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Request<'a> {
    List { path: &'a str },
    Download { path: &'a str },
    Upload { path: &'a str, size: u64 },
}

#[must_use]
pub fn encode(request: &Request<'_>) -> Option<String> {
    let path = match request {
        Request::List { path } | Request::Download { path } | Request::Upload { path, .. } => path,
    };
    if !is_sendable(path) {
        return None;
    }
    let text = serde_json::to_string(request).ok()?;
    (text.len() <= MAX_TEXT).then_some(text)
}

/// Whether the hub takes `path` (K4 Section 9).
#[must_use]
pub fn is_sendable(path: &str) -> bool {
    !path.is_empty() && !path.contains('\0') && path.len() <= MAX_PATH
}

/// Why the hub refused a request (K4 Section 9 `refused.reason`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Refusal {
    DownloadOff,
    UploadOff,
    NotFound,
    Denied,
    Exists,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    File,
    Dir,
    Link,
}

/// One directory entry as the device reported it, with K4's field names.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
    pub size: Option<u64>,
    pub modified_at: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Listing {
    pub path: String,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HubMessage {
    Entries(Listing),
    Download { size: u64 },
    Done,
    Refused(Refusal),
    Ping,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
enum Wire {
    Entries { path: String, entries: Vec<Entry> },
    Download { size: u64 },
    Done {},
    Refused { reason: Refusal },
    Ping {},
}

#[must_use]
pub fn decode(text: &str) -> Option<HubMessage> {
    if !text.trim_start().starts_with('{') {
        return None;
    }
    Some(match serde_json::from_str::<Wire>(text).ok()? {
        Wire::Entries { path, entries } => {
            if path.is_empty() || entries.len() > MAX_ENTRIES {
                return None;
            }
            HubMessage::Entries(Listing { path, entries })
        }
        Wire::Download { size } => HubMessage::Download { size },
        Wire::Done {} => HubMessage::Done,
        Wire::Refused { reason } => HubMessage::Refused(reason),
        Wire::Ping {} => HubMessage::Ping,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn k4_9_requests_follow_the_path_rules() {
        assert_eq!(
            encode(&Request::Upload {
                path: "/tmp/a b",
                size: 3
            })
            .as_deref(),
            Some(r#"{"type":"upload","path":"/tmp/a b","size":3}"#)
        );
        assert_eq!(encode(&Request::List { path: "" }), None);
        assert_eq!(encode(&Request::Download { path: "a\0b" }), None);
        let long = "a".repeat(MAX_PATH + 1);
        assert_eq!(encode(&Request::List { path: &long }), None);
        let quoted = "\"".repeat(MAX_PATH);
        assert_eq!(
            encode(&Request::List { path: &quoted }),
            None,
            "over 4 KiB once escaped"
        );
    }

    #[test]
    fn k4_9_the_hubs_messages_are_a_closed_set() {
        let entries = r#"{"type":"entries","path":"/home/op","entries":[
            {"name":"a","kind":"file","size":3,"modifiedAt":"2026-11-01T09:00:00Z"},
            {"name":"d","kind":"dir","size":null,"modifiedAt":null}]}"#;
        let Some(HubMessage::Entries(listing)) = decode(entries) else {
            panic!("entries");
        };
        assert_eq!(listing.entries.len(), 2);
        assert_eq!(listing.entries[1].kind, Kind::Dir);
        assert_eq!(
            decode(r#"{"type":"download","size":10}"#),
            Some(HubMessage::Download { size: 10 })
        );
        assert_eq!(
            decode(r#"{"type":"refused","reason":"UPLOAD_OFF"}"#),
            Some(HubMessage::Refused(Refusal::UploadOff))
        );
        for text in [
            r#"{"type":"download","size":-1}"#,
            r#"{"type":"download","size":1.5}"#,
            r#"{"type":"refused","reason":"LATER"}"#,
            r#"{"type":"done","x":1}"#,
            r#"{"type":"entries","path":"","entries":[]}"#,
            r#"{"type":"entries","path":"/","entries":[{"name":"s","kind":"socket","size":null,"modifiedAt":null}]}"#,
            r#"{"type":"entries","path":"/","entries":[{"name":"a","kind":"file","size":1,"modifiedAt":null,"mode":7}]}"#,
            r#"[{"type":"done"}]"#,
            "done",
        ] {
            assert_eq!(decode(text), None, "{text}");
        }
    }

    #[test]
    fn k4_9_too_many_entries_are_refused() {
        let entry = r#"{"name":"a","kind":"file","size":1,"modifiedAt":null}"#;
        let many = vec![entry; MAX_ENTRIES + 1].join(",");
        let text = format!(r#"{{"type":"entries","path":"/","entries":[{many}]}}"#);
        assert_eq!(decode(&text), None);
    }

    fn next(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    #[test]
    fn k4_9_no_hub_text_panics_the_decoder() {
        let pieces = [
            "{",
            "}",
            "[",
            "]",
            "\"type\"",
            ":",
            ",",
            "\"entries\"",
            "\"download\"",
            "\"done\"",
            "\"size\"",
            "18446744073709551616",
            "0",
            "\"path\"",
            "\"/\"",
            "null",
            "\u{1b}",
        ];
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        for _ in 0..20_000 {
            let count = next(&mut state) % 14;
            let text: String = (0..count)
                .map(|_| {
                    pieces[usize::try_from(next(&mut state) % pieces.len() as u64).unwrap_or(0)]
                })
                .collect();
            let _decoded = decode(&text);
        }
    }
}
