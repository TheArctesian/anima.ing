//! Only a person's browser token passes the shell gate; an agent header stops it (OD-HOST-54, SF-252).

use axum::Router;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{PUBLIC, browser_hub, browser_token, cookie, post};
use crate::hub::{set_operator, start};

const INSTALL: &str = "mutation($entry: String!, $device: ID!, $digest: String) { \
    installCatalogueEntry(entryId: $entry, deviceId: $device, digest: $digest) { refusal } }";
const DIGESTS: &str = "query($id: String!) { catalogueEntry(id: $id) { digests } }";
const DEVICE: &str = "0190a000-0000-7000-8002-000000000002";

async fn refusal(hub: &Router, headers: &[(&str, &str)], digest: Value) -> Value {
    let variables = json!({ "entry": "code-server", "device": DEVICE, "digest": digest });
    let answer = post(hub, headers, INSTALL, variables).await.json();
    answer["data"]["installCatalogueEntry"]["refusal"].clone()
}

#[sqlx::test(migrations = "./migrations")]
async fn sf_revlsform_a_person_in_the_browser_passes_the_shell_gate_and_names_the_digest(
    pool: PgPool,
) {
    set_operator(&pool).await;
    let hub = browser_hub(&pool, start());
    let token = browser_token(&hub).await;
    let cookie = cookie(&token);
    let person = [("origin", PUBLIC), ("cookie", cookie.as_str())];
    let answer = post(&hub, &person, DIGESTS, json!({ "id": "code-server" }))
        .await
        .json();
    let digest = answer["data"]["catalogueEntry"]["digests"][0].clone();
    assert!(digest.is_string(), "{answer}");
    assert_eq!(
        refusal(&hub, &person, Value::Null).await,
        "GENERATED_NEEDS_DIGEST"
    );
    assert_eq!(
        refusal(&hub, &person, digest.clone()).await,
        "INSTALL_NOT_BUILT"
    );
    let agent = [person[0], person[1], ("anima-actor", "agent")];
    assert_eq!(refusal(&hub, &agent, digest).await, "SHELL_NEEDS_PERSON");
}
