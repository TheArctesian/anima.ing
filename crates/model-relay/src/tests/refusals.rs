//! What is refused before a provider is asked.

use hyper::{Method, StatusCode};

use super::{call, relay, seen};
use crate::upstream::{Program, Upstreams};

#[tokio::test]
async fn sa6_no_secret_no_relay() {
    let (relay, provider) = relay().await;
    let base = relay.open("thread-3", Program::ClaudeCode).expect("open");
    let root = &base[..base.rfind('/').expect("a path")];
    let wrong = format!("{root}/{}", "0".repeat(64));
    for url in [
        format!("{root}/v1/messages"),
        format!("{wrong}/v1/messages"),
        format!("{base}x/v1/messages"),
        root.to_owned(),
    ] {
        let answer = call(Method::POST, &url, &[("authorization", "Bearer x")], "{}").await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{url}");
    }
    relay.close("thread-3");
    let closed = call(Method::POST, &format!("{base}/v1/messages"), &[], "{}").await;
    assert_eq!(
        closed.status,
        StatusCode::NOT_FOUND,
        "a secret dies with its thread"
    );
    assert!(seen(&provider).is_empty(), "nothing went to the provider");
    assert!(relay.open("thread-4", Program::Codex).is_ok());
    assert!(
        relay.open("thread-4", Program::Codex).is_err(),
        "one secret for one thread"
    );
}

#[tokio::test]
async fn sa6_a_web_page_and_an_upgrade_are_refused_before_the_provider() {
    let (relay, provider) = relay().await;
    let base = relay.open("thread-5", Program::ClaudeCode).expect("open");
    let url = format!("{base}/v1/messages");
    let page = call(
        Method::POST,
        &url,
        &[("origin", "https://evil.example")],
        "{}",
    )
    .await;
    assert_eq!(page.status, StatusCode::FORBIDDEN);
    assert!(!page.headers.contains_key("access-control-allow-origin"));
    let upgrade = call(
        Method::GET,
        &url,
        &[("connection", "upgrade"), ("upgrade", "websocket")],
        "",
    )
    .await;
    assert_eq!(upgrade.status, StatusCode::NOT_IMPLEMENTED);
    assert!(seen(&provider).is_empty(), "nothing went to the provider");
}

#[tokio::test]
async fn sa6_a_request_cannot_name_a_different_host_and_no_redirect_is_followed() {
    let (relay, provider) = relay().await;
    let base = relay.open("thread-6", Program::ClaudeCode).expect("open");
    let headers = [
        ("x-headroom-base-url", "http://elsewhere.invalid"),
        ("x-forwarded-host", "elsewhere.invalid"),
        ("forwarded", "host=elsewhere.invalid"),
    ];
    let answer = call(
        Method::POST,
        &format!("{base}//elsewhere.invalid/v1/messages"),
        &headers,
        "{}",
    )
    .await;
    assert_eq!(
        answer.status,
        StatusCode::OK,
        "it went to the one provider, with the path as it was"
    );
    let moved = call(Method::GET, &format!("{base}/v1/moved"), &[], "").await;
    assert_eq!(
        moved.status,
        StatusCode::FOUND,
        "a redirect goes back to the program; the relay does not follow it"
    );
    let got = seen(&provider);
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].path, "//elsewhere.invalid/v1/messages");
    let own = Upstreams::providers();
    let hosts = [&own.anthropic, &own.openai, &own.chatgpt]
        .map(|each| (each.host.as_str(), each.port, each.tls));
    assert_eq!(
        hosts,
        [
            ("api.anthropic.com", 443, true),
            ("api.openai.com", 443, true),
            ("chatgpt.com", 443, true)
        ]
    );
}
