//! Who may make the issue changes and what the record names (K-E1 Section 13.5, D-23, LOG-014
//! to LOG-018); a key's budget and pace (K-E1 Section 13.1).

use axum::{
    body::Body,
    http::{Request, header},
};
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{COMMENT, CREATE, SET_OPEN, answer, audits, comment, sent};
use crate::{
    hub::{error, start, unauthenticated},
    projects::forge::{BASE, Forge, forge, issue, reply},
};

async fn key(forge: &Forge, scope: &str, made_for: &str) -> String {
    let query = "mutation($input: ApiKeyInput!) { createApiKey(input: $input) { secret } }";
    let input = json!({ "input": { "name": format!("{scope}-{made_for}"), "scope": scope,
        "madeFor": made_for, "expiresAt": start() + TimeDelta::days(30) } });
    let response = forge.kit.post(query, input).await;
    response["data"]["createApiKey"]["secret"]
        .as_str()
        .unwrap_or_else(|| panic!("no key: {response}"))
        .to_owned()
}

fn script(forge: &Forge) {
    answer(forge, ("POST", "/issues"), 201, &json!({ "number": 7 }));
    forge.ok("/issues/7", &issue(7));
    answer(
        forge,
        ("POST", "/issues/5/comments"),
        201,
        &json!({ "id": 9001 }),
    );
    forge.ok("/issues/5", &issue(5));
    forge.ok("/issues/5/comments", &json!([comment(9001, 5, "Seen it.")]));
    forge.ok("/issues/3", &issue(3));
    answer(forge, ("PATCH", "/issues/3"), 200, &issue(3));
}

fn calls(forge: &Forge) -> [(&'static str, &'static str, Value); 3] {
    let with = |mut variables: Value| {
        variables["repository"] = json!(forge.repository);
        variables
    };
    [
        (
            "createIssue",
            CREATE,
            with(json!({ "title": "Printer jams" })),
        ),
        (
            "addIssueComment",
            COMMENT,
            with(json!({ "number": 5, "body": "Seen it." })),
        ),
        (
            "setIssueOpen",
            SET_OPEN,
            with(json!({ "number": 3, "open": false })),
        ),
    ]
}

/// Declares an agent with the `anima-actor` header (LOG-016).
async fn as_agent(forge: &Forge, token: &str, query: &str, variables: Value) -> Value {
    let body = json!({ "query": query, "variables": variables }).to_string();
    let request = Request::post("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header("anima-actor", "agent")
        .body(Body::from(body))
        .unwrap();
    let (status, text) = forge.kit.hub.send(request).await;
    assert!(status.is_success(), "{status} {text}");
    serde_json::from_str(&text).unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn d_23_the_issue_changes_take_a_login_token_and_a_write_key_and_say_who(pool: PgPool) {
    let forge = forge(&pool).await;
    script(&forge);
    let callers = [
        (forge.kit.token.clone(), "user", "login"),
        (key(&forge, "WRITE", "USER").await, "user", "api_key"),
        (key(&forge, "WRITE", "AGENT").await, "agent", "api_key"),
    ];
    for (token, actor, kind) in &callers {
        for (root, query, variables) in calls(&forge) {
            forge.clock.advance(2);
            let response = forge.kit.hub.post(Some(token), query, variables).await;
            assert!(response["errors"].is_null(), "{root} {kind}: {response}");
            assert_ne!(response["data"][root], Value::Null, "{root}");
            let entries = audits(&pool).await;
            let last = entries.last().unwrap();
            assert_eq!(
                (&last["actor_kind"], &last["token_kind"]),
                (&json!(actor), &json!(kind)),
                "{root}"
            );
        }
    }
    for (root, query, variables) in calls(&forge) {
        forge.clock.advance(2);
        let response = as_agent(&forge, &forge.kit.token, query, variables).await;
        assert!(response["errors"].is_null(), "{root} declared: {response}");
        let entries = audits(&pool).await;
        assert_eq!(entries.last().unwrap()["actor_kind"], "agent", "{root}");
    }
    assert_eq!(audits(&pool).await.len(), 12);
}

#[sqlx::test(migrations = "./migrations")]
async fn d_23_a_read_key_and_a_signed_out_request_change_nothing(pool: PgPool) {
    let forge = forge(&pool).await;
    script(&forge);
    let read = key(&forge, "READ", "USER").await;
    for (root, query, variables) in calls(&forge) {
        let refused = forge
            .kit
            .hub
            .post(Some(&read), query, variables.clone())
            .await;
        assert_eq!(error(&refused).1, "FORBIDDEN", "{root}: {refused}");
        let signed_out = forge.kit.hub.post(None, query, variables).await;
        assert_eq!(error(&signed_out), unauthenticated(), "{root}");
    }
    assert!(sent(&forge, "POST", "/issues").is_empty(), "nothing");
    assert!(
        sent(&forge, "POST", "/issues/5/comments").is_empty(),
        "nothing"
    );
    assert!(sent(&forge, "PATCH", "/issues/3").is_empty(), "nothing");
    assert!(audits(&pool).await.is_empty(), "nothing");
}

#[sqlx::test(migrations = "./migrations")]
async fn od_e14_a_key_change_stops_at_the_reserve_and_a_login_change_does_not(pool: PgPool) {
    let forge = forge(&pool).await;
    script(&forge);
    let reset = (start() + TimeDelta::minutes(20)).timestamp().to_string();
    let left = [
        ("x-ratelimit-remaining", "500"),
        ("x-ratelimit-reset", reset.as_str()),
    ];
    let made = json!({ "number": 7 }).to_string();
    let path = format!("{BASE}/issues");
    (forge.kit.stub).reply_to("POST", &path, reply(201, &left, &made));
    let write = key(&forge, "WRITE", "USER").await;
    let [(root, query, variables), ..] = calls(&forge);
    forge.kit.post(query, variables.clone()).await;
    assert_eq!(sent(&forge, "POST", "/issues").len(), 1, "{root}");

    forge.clock.advance(2);
    let refused = forge
        .kit
        .hub
        .post(Some(&write), query, variables.clone())
        .await;
    assert_eq!(
        error(&refused),
        (
            "GitHub limits requests until 12:20.".to_owned(),
            "UNAVAILABLE".to_owned()
        )
    );
    assert_eq!(
        sent(&forge, "POST", "/issues").len(),
        1,
        "the key's change was not sent"
    );

    forge.clock.advance(2);
    forge.kit.post(query, variables).await;
    assert_eq!(
        sent(&forge, "POST", "/issues").len(),
        2,
        "the login's change went"
    );
    assert_eq!(
        audits(&pool).await.len(),
        2,
        "the refused change records nothing"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn od_e14_a_key_makes_at_most_20_changes_a_minute(pool: PgPool) {
    let forge = forge(&pool).await;
    script(&forge);
    let write = key(&forge, "WRITE", "USER").await;
    let [(_, query, variables), ..] = calls(&forge);
    for n in 1..=20 {
        forge.clock.advance(2);
        let response = forge
            .kit
            .hub
            .post(Some(&write), query, variables.clone())
            .await;
        assert!(response["errors"].is_null(), "change {n}: {response}");
    }

    forge.clock.advance(2);
    let refused = forge
        .kit
        .hub
        .post(Some(&write), query, variables.clone())
        .await;
    assert_eq!(
        error(&refused),
        (
            "This key makes at most 20 changes a minute. Try again in a minute.".to_owned(),
            "UNAVAILABLE".to_owned()
        )
    );
    assert_eq!(sent(&forge, "POST", "/issues").len(), 20);
    let login = forge.kit.post(query, variables.clone()).await;
    assert!(login["errors"].is_null(), "{login}");

    forge.clock.advance(60);
    let again = forge.kit.hub.post(Some(&write), query, variables).await;
    assert!(again["errors"].is_null(), "{again}");
    assert_eq!(sent(&forge, "POST", "/issues").len(), 22);
    assert_eq!(audits(&pool).await.len(), 22);
}
