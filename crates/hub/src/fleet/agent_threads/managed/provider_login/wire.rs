//! Provider-specific live frames; arbitrary provider output never reaches clients.
use super::{Lease, targets};
use serde::Deserialize;
use serde_json::json;
use std::{collections::BTreeMap, time::Duration};
use tokio::io::{AsyncWriteExt, WriteHalf};
use zeroize::Zeroizing;

#[derive(Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Event {
    Challenge {
        verification_url: String,
        code: String,
    },
    PasteChallenge {
        verification_url: String,
    },
    Succeeded,
    Failed,
    Cancelled,
    Expired,
}

pub(super) async fn receive(reply: &mut Option<Lease>) -> Option<Zeroizing<String>> {
    match reply {
        Some(lease) => lease.receive.recv().await,
        None => std::future::pending().await,
    }
}

pub(super) async fn send(
    pipe: &mut WriteHalf<targets::Reached>,
    code: &str,
    deadline: tokio::time::Instant,
) -> std::io::Result<()> {
    let mut frame =
        Zeroizing::new(serde_json::to_vec(&json!({"code":code})).map_err(std::io::Error::other)?);
    frame.push(b'\n');
    tokio::time::timeout_at(
        deadline.min(tokio::time::Instant::now() + Duration::from_secs(5)),
        async {
            pipe.write_all(&frame).await?;
            pipe.flush().await
        },
    )
    .await
    .map_err(|_| std::io::Error::other("provider reply timed out"))?
}

pub(super) fn claude_url(raw: &str) -> bool {
    if raw.len() > 2048 || !raw.is_ascii() {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(raw) else {
        return false;
    };
    if url.scheme() != "https"
        || url.host_str() != Some("claude.com")
        || url.path() != "/cai/oauth/authorize"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
        || raw
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return false;
    }
    let mut query = BTreeMap::new();
    for (key, value) in url.query_pairs() {
        if query.insert(key, value).is_some() {
            return false;
        }
    }
    [
        ("client_id", "9d1c250a-e61b-44d9-88ed-5944d1962f5e"),
        ("response_type", "code"),
        (
            "redirect_uri",
            "https://platform.claude.com/oauth/code/callback",
        ),
        ("code_challenge_method", "S256"),
    ]
    .into_iter()
    .all(|(key, value)| query.get(key).is_some_and(|actual| actual == value))
        && ["state", "code_challenge"]
            .into_iter()
            .all(|key| query.get(key).is_some_and(|value| !value.is_empty()))
}
