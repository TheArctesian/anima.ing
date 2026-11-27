//! Pinned Claude login URL and returned-code protocol; raw terminal output stays local.
use std::{
    collections::BTreeMap,
    io::{BufRead, Read},
};
use zeroize::Zeroizing;

const AUTHORIZE: &str = "https://claude.com/cai/oauth/authorize";

#[cfg(test)]
#[path = "claude_challenge_tests.rs"]
mod tests;

pub(super) fn challenge(bytes: &[u8]) -> Option<Zeroizing<String>> {
    let start = bytes
        .windows(AUTHORIZE.len())
        .position(|part| part == AUTHORIZE.as_bytes())?;
    let remaining = &bytes[start..];
    // Both ordinary text and OSC 8 hyperlinks terminate before these bytes.
    let end = remaining
        .iter()
        .position(|byte| byte.is_ascii_whitespace() || *byte == 0x1b || *byte == 7)?;
    let raw = std::str::from_utf8(remaining.get(..end)?).ok()?;
    if raw.len() > 2048 || !raw.is_ascii() {
        return None;
    }
    let url = reqwest::Url::parse(raw).ok()?;
    if url.scheme() != "https"
        || url.host_str() != Some("claude.com")
        || url.path() != "/cai/oauth/authorize"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    let mut query = BTreeMap::new();
    for (key, value) in url.query_pairs() {
        if query.insert(key, value).is_some() {
            return None;
        }
    }
    for (key, expected) in [
        ("client_id", "9d1c250a-e61b-44d9-88ed-5944d1962f5e"),
        ("response_type", "code"),
        (
            "redirect_uri",
            "https://platform.claude.com/oauth/code/callback",
        ),
        ("code_challenge_method", "S256"),
    ] {
        if query.get(key).map(AsRef::as_ref) != Some(expected) {
            return None;
        }
    }
    for key in ["state", "code_challenge"] {
        if query.get(key).is_none_or(|value| value.is_empty()) {
            return None;
        }
    }
    Some(Zeroizing::new(raw.to_owned()))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Reply {
    code: String,
}

pub(super) fn replies() -> tokio::sync::mpsc::Receiver<Zeroizing<String>> {
    let (send, receive) = tokio::sync::mpsc::channel(1);
    // Native blocking input must not keep Tokio's runtime alive after cancellation.
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        let mut bytes = Zeroizing::new(Vec::new());
        if input
            .by_ref()
            .take(2049)
            .read_until(b'\n', &mut bytes)
            .is_err()
            || bytes.len() > 2048
            || bytes.last() != Some(&b'\n')
        {
            return;
        }
        let Ok(reply) = serde_json::from_slice::<Reply>(&bytes) else {
            return;
        };
        let code = Zeroizing::new(reply.code);
        if !agent_threads::managed::provider_login::valid_code(&code) {
            return;
        }
        if send.blocking_send(code).is_err() {
            return;
        }
        // Any second input or EOF closes the receiver and cancels the live worker.
        let _ = input.read(&mut [0_u8; 1]);
    });
    receive
}
