//! What passes unchanged, and to which host.

use hyper::{Method, StatusCode};

use super::{call, relay, seen};
use crate::upstream::Program;

#[tokio::test]
async fn sa6_a_request_and_its_answer_pass_unchanged() {
    let (relay, provider) = relay().await;
    let base = relay.open("thread-1", Program::ClaudeCode).expect("open");
    assert!(
        base.starts_with("http://127.0.0.1:"),
        "the address is the loopback address"
    );
    let headers = [
        ("authorization", "Bearer sign-in-of-the-test"),
        ("anthropic-version", "2023-06-01"),
        ("x-custom", "kept"),
        ("connection", "x-hop"),
        ("x-hop", "dropped"),
    ];
    let answer = call(
        Method::POST,
        &format!("{base}/v1/messages?beta=true"),
        &headers,
        r#"{"messages":[1]}"#,
    )
    .await;
    assert_eq!(answer.status, StatusCode::OK);
    assert_eq!(answer.body, r#"answer to {"messages":[1]}"#);
    assert_eq!(answer.headers["x-provider"], "yes");
    let got = seen(&provider);
    assert_eq!(got.len(), 1);
    assert_eq!(
        (got[0].method.as_str(), got[0].path.as_str()),
        ("POST", "/v1/messages?beta=true")
    );
    assert_eq!(got[0].body, r#"{"messages":[1]}"#);
    assert_eq!(
        got[0].headers["authorization"],
        "Bearer sign-in-of-the-test"
    );
    assert_eq!(got[0].headers["anthropic-version"], "2023-06-01");
    assert_eq!(got[0].headers["x-custom"], "kept");
    assert!(
        !got[0].headers.contains_key("x-hop"),
        "a header of one connection does not pass"
    );
    assert!(
        !got[0].headers.contains_key("x-forwarded-for"),
        "the relay adds no header of its own"
    );
}

#[tokio::test]
async fn sa6_codex_goes_to_the_host_of_its_sign_in_kind() {
    let (relay, provider) = relay().await;
    let base = relay.open("thread-2", Program::Codex).expect("open");
    assert!(base.ends_with("/v1"), "{base}");
    let secret = base.split('/').nth(3).expect("secret");
    let address = base.replace(&format!("/{secret}"), "");
    call(
        Method::POST,
        &format!("{address}/responses"),
        &[("authorization", "Bearer k"), ("x-anima-relay", secret)],
        "{}",
    )
    .await;
    call(
        Method::POST,
        &format!("{address}/responses"),
        &[
            ("authorization", "Bearer t"),
            ("chatgpt-account-id", "a"),
            ("x-anima-relay", secret),
        ],
        "{}",
    )
    .await;
    let got = seen(&provider);
    assert!(
        got.iter()
            .all(|request| !request.headers.contains_key("x-anima-relay"))
    );
    assert_eq!(got[0].headers["authorization"], "Bearer k");
    assert_eq!(got[1].headers["authorization"], "Bearer t");
    assert_eq!(got[1].headers["chatgpt-account-id"], "a");
    let paths: Vec<String> = got.into_iter().map(|got| got.path).collect();
    assert_eq!(paths, ["/v1/responses", "/backend-api/codex/responses"]);
    let outside = call(Method::GET, &base.replace("/v1", "/v2/responses"), &[], "").await;
    assert_eq!(outside.status, StatusCode::NOT_FOUND);
}
