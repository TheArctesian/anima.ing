//! Who may add a public route (SF-330): only a person in the Anima app in a
//! browser, with no agent declared. A login token, which the `anima` command and an agent hold,
//! the same token declared as an agent (MCP), and an API key are refused; a private route keeps
//! the rule of a login token. No route is stored: the account has no second factor, so even the
//! person stops at that check. Removing a route needs only a signed-in person, never an API key,
//! and leaves one audit entry that names what it took away (ruling of 2026-11-22).

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

const REMOVE: &str = "mutation($id: ID!) { removeApplicationRoute(id: $id) }";
const DOCKER_HOST: &str = "0190a000-0000-7000-8002-000000000002";
const PRIVATE_ROUTE: &str = "0190a000-0000-7000-8030-000000000001";
const PUBLIC_ROUTE: &str = "0190a000-0000-7000-8030-000000000002";

/// Stores a route of `access` on the docker host, as an addition would.
async fn stored_route(pool: &PgPool, id: &str, name: &str, access: &str) {
    sqlx::query(
        "INSERT INTO routes.route (id, name, application_id, device_id, port, token, access, \
         created_at) VALUES ($1::text::uuid, $2, gen_random_uuid(), $3::text::uuid, 3000, \
         repeat('a', 64), $4, now())",
    )
    .bind(id)
    .bind(name)
    .bind(DOCKER_HOST)
    .bind(access)
    .execute(pool)
    .await
    .unwrap();
}

async fn routes_left(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT name FROM routes.route ORDER BY name")
        .fetch_all(pool)
        .await
        .unwrap()
}

/// The `change` of each removal in the audit log of the docker host.
async fn removals(pool: &PgPool) -> Vec<Value> {
    sqlx::query_scalar(
        "SELECT change FROM events.audit_log WHERE target_id = $1 AND action LIKE '%delete%' \
         ORDER BY at",
    )
    .bind(DOCKER_HOST)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn sf_revroutestepup_a_signed_in_person_removes_a_route_with_no_second_factor(pool: PgPool) {
    set_operator(&pool).await;
    stored_route(&pool, PRIVATE_ROUTE, "nas.example.com", "private").await;
    stored_route(&pool, PUBLIC_ROUTE, "photos.example.com", "public").await;
    let hub = browser_hub(&pool, start());
    let login = Hub::new(&pool, start());
    let token = login.token().await;

    let key = login
        .post(
            Some(&token),
            CREATE_KEY,
            json!({ "input": { "name": "agent", "scope": "WRITE", "madeFor": "AGENT",
                "expiresAt": start() + chrono::TimeDelta::days(30) } }),
        )
        .await;
    let key = format!(
        "Bearer {}",
        key["data"]["createApiKey"]["secret"].as_str().unwrap()
    );
    let refused = post(&hub, &[("authorization", key.as_str())], REMOVE, json!({ "id": PRIVATE_ROUTE }))
        .await
        .json();
    assert_eq!(refused["errors"][0]["extensions"]["code"], "FORBIDDEN", "{refused}");
    assert_eq!(routes_left(&pool).await.len(), 2, "an API key removes nothing");

    // The account has no second factor: removing needs only the signed-in person.
    let cookie = cookie(&browser_token(&hub).await);
    let person = [("origin", PUBLIC), ("cookie", cookie.as_str())];
    let removed = post(&hub, &person, REMOVE, json!({ "id": PRIVATE_ROUTE }))
        .await
        .json();
    assert_eq!(removed["data"]["removeApplicationRoute"], PRIVATE_ROUTE, "{removed}");

    let bearer = format!("Bearer {token}");
    let by_login = [("authorization", bearer.as_str())];
    let removed = post(&hub, &by_login, REMOVE, json!({ "id": PUBLIC_ROUTE }))
        .await
        .json();
    assert_eq!(removed["data"]["removeApplicationRoute"], PUBLIC_ROUTE, "{removed}");
    assert!(routes_left(&pool).await.is_empty());

    let again = post(&hub, &person, REMOVE, json!({ "id": PRIVATE_ROUTE }))
        .await
        .json();
    assert_eq!(again["errors"][0]["extensions"]["code"], "NOT_FOUND", "{again}");

    let entries = removals(&pool).await;
    assert_eq!(entries.len(), 2, "one audit entry for each removal, none for a refusal");
    assert_eq!(entries[0]["name"], "nas.example.com");
    assert_eq!(entries[0]["access"], "private");
    assert_eq!(entries[0]["port"], 3000);
    assert_eq!(entries[1]["access"], "public");
    assert!(
        entries.iter().all(|entry| entry.get("token").is_none()),
        "the proof token is never in the log (LOG-007)"
    );
}
