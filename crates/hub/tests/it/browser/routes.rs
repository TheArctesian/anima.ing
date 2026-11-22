//! Who may add a public route (SF-330): only a person in the Anima app in a
//! browser, with no agent declared. A login token, which the `anima` command and an agent hold,
//! the same token declared as an agent (MCP), and an API key are refused; a private route keeps
//! the rule of a login token. No route is stored: the account has no second factor, so even the
//! person stops at that check.

use axum::Router;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{PUBLIC, browser_hub, browser_token, cookie, post};
use crate::hub::{Hub, set_operator, start};

const ADD: &str = "mutation($input: ApplicationRouteInput!) { \
    addApplicationRoute(input: $input) { route { id } } }";
const CREATE_KEY: &str =
    "mutation($input: ApiKeyInput!) { createApiKey(input: $input) { secret } }";
const NEEDS_PERSON: &str = "only by a person in the Anima app in a web browser";
const NEEDS_SECOND_FACTOR: &str = "Turn on the second factor";

/// The message and the code of the one error of adding a route of `access` with `headers`.
async fn refusal(hub: &Router, headers: &[(&str, &str)], access: &str) -> (String, String) {
    let input = json!({ "name": "photos.example.com", "port": 3000, "access": access,
        "applicationId": "0190a000-0000-7000-8009-000000000001",
        "deviceId": "0190a000-0000-7000-8002-000000000002" });
    let answer = post(hub, headers, ADD, json!({ "input": input }))
        .await
        .json();
    assert_eq!(answer["data"], Value::Null, "{answer}");
    let error = &answer["errors"][0];
    (
        error["message"].as_str().unwrap_or_default().to_owned(),
        error["extensions"]["code"]
            .as_str()
            .unwrap_or_default()
            .to_owned(),
    )
}

#[sqlx::test(migrations = "./migrations")]
async fn sf_revroutestepup_only_a_person_in_the_browser_adds_a_public_route(pool: PgPool) {
    set_operator(&pool).await;
    let hub = browser_hub(&pool, start());
    let cookie = cookie(&browser_token(&hub).await);
    let person = [("origin", PUBLIC), ("cookie", cookie.as_str())];
    let (message, _) = refusal(&hub, &person, "PUBLIC").await;
    if !cfg!(unix) {
        // A hub that is not on Unix refuses each route before it asks who adds it (R-366).
        assert!(
            message.contains("Routing needs a hub on Linux."),
            "{message}"
        );
        return;
    }
    assert!(
        message.contains(NEEDS_SECOND_FACTOR),
        "the person passes: {message}"
    );
    let agent = [person[0], person[1], ("anima-actor", "agent")];
    let (message, code) = refusal(&hub, &agent, "PUBLIC").await;
    assert!(
        message.contains(NEEDS_PERSON) && code == "FORBIDDEN",
        "{message}"
    );

    let login = Hub::new(&pool, start()).token().await;
    let login = format!("Bearer {login}");
    for (holder, headers) in [
        ("login token", vec![("authorization", login.as_str())]),
        (
            "MCP",
            vec![("authorization", login.as_str()), ("anima-actor", "agent")],
        ),
    ] {
        let (message, code) = refusal(&hub, &headers, "PUBLIC").await;
        assert!(
            message.contains(NEEDS_PERSON) && code == "FORBIDDEN",
            "{holder}: {message}"
        );
        let (message, _) = refusal(&hub, &headers, "PRIVATE").await;
        assert!(
            message.contains(NEEDS_SECOND_FACTOR),
            "{holder} private: {message}"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn sf_revroutestepup_an_api_key_adds_no_route(pool: PgPool) {
    set_operator(&pool).await;
    let hub = browser_hub(&pool, start());
    let login = Hub::new(&pool, start());
    let token = login.token().await;
    let input = json!({ "name": "agent", "scope": "WRITE", "madeFor": "AGENT",
        "expiresAt": start() + chrono::TimeDelta::days(30) });
    let made = login
        .post(Some(&token), CREATE_KEY, json!({ "input": input }))
        .await;
    let key = format!(
        "Bearer {}",
        made["data"]["createApiKey"]["secret"].as_str().unwrap()
    );
    for access in ["PUBLIC", "PRIVATE"] {
        let (message, code) = refusal(&hub, &[("authorization", key.as_str())], access).await;
        assert_eq!(code, "FORBIDDEN", "{access}: {message}");
        assert!(
            !message.contains(NEEDS_SECOND_FACTOR),
            "{access}: {message}"
        );
    }
}
