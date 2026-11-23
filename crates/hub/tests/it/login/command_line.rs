//! The command-line token of one sign-in (R-383; K-E5 Section 3b, SGN-NEW-linked): only the
//! app's login token of a person asks for it, it acts as the person, a new one ends the one
//! before, and it ends with the app's token, each change with its audit entry and no token in
//! an entry (LOG-007).

use axum::{
    body::Body,
    http::{Request, header},
};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::hub::{Hub, error, set_operator, start, unauthenticated};

const ISSUE: &str = "mutation($c: String!) { issueCommandLineToken(computer: $c) { token expiresAt operator { username } } }";
const ME: &str = "{ me { username } }";
const LOGOUT: &str = "mutation { logout }";

/// A hub at `start()` with the fixture operator and the app's login token.
async fn signed_in(pool: &PgPool) -> (Hub, String) {
    set_operator(pool).await;
    let hub = Hub::new(pool, start());
    let token = hub.token().await;
    (hub, token)
}

/// The command-line token that `token` asks for, for the computer `computer`.
async fn issued(hub: &Hub, token: &str, computer: &str) -> String {
    let response = hub.post(Some(token), ISSUE, json!({ "c": computer })).await;
    response["data"]["issueCommandLineToken"]["token"]
        .as_str()
        .unwrap_or_else(|| panic!("no token: {response}"))
        .to_owned()
}

fn signed_in_as_operator(response: &Value) -> bool {
    response["data"]["me"]["username"] == "operator"
}

fn code(response: &Value) -> &Value {
    &response["errors"][0]["extensions"]["code"]
}

/// Each audit entry: its action, its token kind and its change, in order.
async fn entries(pool: &PgPool) -> Vec<(String, Option<String>, Value)> {
    sqlx::query_as("select action, token_kind, change from events.audit_log order by at, id")
        .fetch_all(pool)
        .await
        .unwrap()
}

#[sqlx::test(migrations = "./migrations")]
async fn sgn_linked_the_app_token_gets_a_command_line_token_that_acts_as_the_person(pool: PgPool) {
    let (hub, app) = signed_in(&pool).await;
    let response = hub
        .post(Some(&app), ISSUE, json!({ "c": " studio " }))
        .await;
    let issued = &response["data"]["issueCommandLineToken"];
    assert_eq!(issued["operator"]["username"], "operator", "{response}");
    // R-033: compare instants, not text. It ends with the app's token.
    let expires_at: chrono::DateTime<chrono::Utc> =
        issued["expiresAt"].as_str().unwrap().parse().unwrap();
    assert_eq!(expires_at, start() + chrono::TimeDelta::days(30));
    let token = issued["token"].as_str().unwrap();
    assert!(signed_in_as_operator(
        &hub.post(Some(token), ME, json!({})).await
    ));

    let (kind, computer, linked): (String, String, bool) = sqlx::query_as(
        "select kind, computer, parent_id is not null from access.api_tokens \
         where kind = 'command_line'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (kind.as_str(), computer.as_str(), linked),
        ("command_line", "studio", true)
    );
    let rows = entries(&pool).await;
    let last = rows.last().unwrap();
    assert_eq!(last.0, "login");
    assert_eq!(
        last.2,
        json!({ "commandLine": "studio", "linkedTo": "app" })
    );
    let text: Vec<String> =
        sqlx::query_scalar("select row_to_json(a)::text from events.audit_log a")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(
        text.iter()
            .all(|row| !row.contains(token) && !row.contains(&app))
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn sgn_linked_a_command_line_token_is_recorded_as_its_own_kind(pool: PgPool) {
    let (hub, app) = signed_in(&pool).await;
    let token = issued(&hub, &app, "studio").await;
    let key = hub
        .post(
            Some(&token),
            "mutation($i: ApiKeyInput!) { createApiKey(input: $i) { key { id } } }",
            json!({ "i": { "name": "script", "scope": "READ", "madeFor": "USER",
                "expiresAt": start() + chrono::TimeDelta::days(1) } }),
        )
        .await;
    assert!(key["errors"].is_null(), "it acts as the person: {key}");
    let rows = entries(&pool).await;
    assert_eq!(rows.last().unwrap().1.as_deref(), Some("command_line"));
}

#[sqlx::test(migrations = "./migrations")]
async fn sgn_linked_the_sign_out_of_the_app_ends_its_command_line_token(pool: PgPool) {
    let (hub, app) = signed_in(&pool).await;
    let token = issued(&hub, &app, "studio").await;
    assert_eq!(
        hub.post(Some(&app), LOGOUT, json!({})).await["data"]["logout"],
        true
    );

    assert_eq!(
        error(&hub.post(Some(&token), ME, json!({})).await),
        unauthenticated()
    );
    let count: i64 = sqlx::query_scalar("select count(*) from access.api_tokens")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    let rows = entries(&pool).await;
    let actions: Vec<&str> = rows.iter().map(|row| row.0.as_str()).collect();
    assert_eq!(actions, ["login", "login", "logout", "logout"]);
    assert_eq!(rows[2].2, json!({ "commandLine": "studio", "with": "app" }));
}

#[sqlx::test(migrations = "./migrations")]
async fn sgn_linked_the_sign_out_of_the_command_line_ends_only_its_own_token(pool: PgPool) {
    let (hub, app) = signed_in(&pool).await;
    let token = issued(&hub, &app, "studio").await;
    assert_eq!(
        hub.post(Some(&token), LOGOUT, json!({})).await["data"]["logout"],
        true
    );

    assert_eq!(
        error(&hub.post(Some(&token), ME, json!({})).await),
        unauthenticated()
    );
    assert!(signed_in_as_operator(
        &hub.post(Some(&app), ME, json!({})).await
    ));
}

#[sqlx::test(migrations = "./migrations")]
async fn sgn_linked_a_new_command_line_token_ends_the_one_before(pool: PgPool) {
    let (hub, app) = signed_in(&pool).await;
    let first = issued(&hub, &app, "studio").await;
    let second = issued(&hub, &app, "studio").await;

    assert_eq!(
        error(&hub.post(Some(&first), ME, json!({})).await),
        unauthenticated()
    );
    assert!(signed_in_as_operator(
        &hub.post(Some(&second), ME, json!({})).await
    ));
}

#[sqlx::test(migrations = "./migrations")]
async fn sgn_linked_only_the_app_token_of_a_person_asks_for_one(pool: PgPool) {
    let (hub, app) = signed_in(&pool).await;
    let token = issued(&hub, &app, "studio").await;
    let from_command_line = hub.post(Some(&token), ISSUE, json!({ "c": "other" })).await;
    assert_eq!(code(&from_command_line), "FORBIDDEN", "{from_command_line}");

    let body = json!({ "query": ISSUE, "variables": { "c": "agent" } }).to_string();
    let request = Request::post("/graphql")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {app}"))
        .header("anima-actor", "agent")
        .body(Body::from(body))
        .unwrap();
    let (_, text) = hub.send(request).await;
    let from_agent: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(code(&from_agent), "FORBIDDEN", "{from_agent}");

    let blank = hub.post(Some(&app), ISSUE, json!({ "c": "  " })).await;
    assert_eq!(code(&blank), "VALIDATION", "{blank}");
    assert!(signed_in_as_operator(
        &hub.post(Some(&token), ME, json!({})).await
    ));
}
