//! Repository credentials use the connected App key and the normal GitHub transport.

use std::num::NonZeroU64;

use anima_hub::engine::sources::{
    Keep, LiveFailure, ask, mint_repository_checkout, revoke_repository_checkout,
};
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{PULLS, get, live_kit, ok, reply};
use crate::hub::start;

const MINT: &str = "/app/installations/77/access_tokens";
const REVOKE: &str = "/installation/token";
const CHECKOUT_TOKEN: &str = "fixture-checkout";
const REPOSITORY_ID: NonZeroU64 = NonZeroU64::new(127).unwrap();

#[sqlx::test(migrations = "./migrations")]
async fn a_checkout_uses_one_repository_and_leaves_the_installation_cache_unchanged(pool: PgPool) {
    let (kit, account) = live_kit(&pool).await;
    let scheduling = kit.at(start());
    kit.stub.reply(PULLS, ok("[]"));
    ask(&scheduling, account, &get(PULLS), Keep::for_seconds(0))
        .await
        .unwrap();
    let before = kit.stub.with(|script| script.heard.len());
    let expires = start() + TimeDelta::hours(1);
    kit.stub
        .reply_once("POST", MINT, reply(201, &[], &scope(expires).to_string()));
    kit.stub.reply_to("DELETE", REVOKE, reply(204, &[], ""));
    let checkout = mint_repository_checkout(&scheduling, account, REPOSITORY_ID)
        .await
        .unwrap();
    assert_eq!(checkout.value.expose(), CHECKOUT_TOKEN.as_bytes());
    assert_eq!(checkout.expires_at, expires);
    revoke_repository_checkout(&scheduling.transport, &checkout.value)
        .await
        .unwrap();

    let issues = "/repos/octo/widgets/issues";
    kit.stub.reply(issues, ok("[]"));
    ask(&scheduling, account, &get(issues), Keep::for_seconds(0))
        .await
        .unwrap();
    let installation = format!("Bearer {}", kit.stub.token());
    kit.stub.with(|script| {
        let [mint, revoke, read] = &script.heard[before..] else {
            panic!("expected one mint, one revocation and one live read")
        };
        assert_eq!((&*mint.method, &*mint.path), ("POST", MINT));
        assert_eq!(mint.header("content-type"), [b"application/json"]);
        assert_eq!(
            serde_json::from_slice::<Value>(&mint.body).unwrap(),
            json!({"repository_ids":[127],"permissions":{"contents":"read"}})
        );
        assert_eq!((&*revoke.method, &*revoke.path), ("DELETE", REVOKE));
        assert_eq!(revoke.header("authorization"), [b"Bearer fixture-checkout"]);
        assert_eq!((&*read.method, &*read.path), ("GET", issues));
        assert_eq!(read.header("authorization"), [installation.as_bytes()]);
    });
}

#[sqlx::test(migrations = "./migrations")]
async fn a_checkout_refuses_and_revokes_a_token_without_proof_of_narrow_scope(pool: PgPool) {
    let (kit, account) = live_kit(&pool).await;
    let scheduling = kit.at(start());
    kit.stub.reply_to("DELETE", REVOKE, reply(204, &[], ""));
    let before = kit.stub.with(|script| script.heard.len());
    let expires = start() + TimeDelta::hours(1);
    let accepted = scope(expires);
    let cases = [
        ("repository_selection", json!("all")),
        ("repositories", json!([{"id":128}])),
        ("repositories", json!([{"id":127},{"id":128}])),
        ("repositories", Value::Null),
        ("permissions", json!({"contents":"read","issues":"read"})),
        ("permissions", json!({"contents":"write"})),
        ("expires_at", json!(start())),
        ("expires_at", json!(expires + TimeDelta::seconds(1))),
    ];
    for (field, value) in &cases {
        let mut refused = accepted.clone();
        refused[field] = value.clone();
        kit.stub
            .reply_once("POST", MINT, reply(201, &[], &refused.to_string()));
        let result = mint_repository_checkout(&scheduling, account, REPOSITORY_ID).await;
        assert!(
            matches!(result, Err(LiveFailure::Unusable)),
            "{field}: {value}"
        );
    }
    kit.stub.with(|script| {
        let requests = &script.heard[before..];
        assert_eq!(requests.len(), cases.len() * 2);
        for [mint, revoke] in requests.as_chunks::<2>().0 {
            assert_eq!((&*mint.method, &*mint.path), ("POST", MINT));
            assert_eq!((&*revoke.method, &*revoke.path), ("DELETE", REVOKE));
            assert_eq!(revoke.header("authorization"), [b"Bearer fixture-checkout"]);
        }
    });
}

#[sqlx::test(migrations = "./migrations")]
async fn a_checkout_token_is_revoked_if_the_account_pauses_during_minting(pool: PgPool) {
    let (kit, account) = live_kit(&pool).await;
    let expires = start() + TimeDelta::hours(1);
    kit.stub
        .reply_once("POST", MINT, reply(201, &[], &scope(expires).to_string()));
    kit.stub.reply_to("DELETE", REVOKE, reply(204, &[], ""));
    kit.stub.hook(
        MINT,
        &format!("UPDATE sources.accounts SET paused_at = now() WHERE id = '{account}'"),
    );
    let before = kit.stub.with(|script| script.heard.len());
    let result = mint_repository_checkout(&kit.at(start()), account, REPOSITORY_ID).await;

    assert!(matches!(result, Err(LiveFailure::Paused)));
    kit.stub.with(|script| {
        let [mint, revoke] = &script.heard[before..] else {
            panic!("expected the paused account's mint to be revoked")
        };
        assert_eq!((&*mint.method, &*mint.path), ("POST", MINT));
        assert_eq!((&*revoke.method, &*revoke.path), ("DELETE", REVOKE));
        assert_eq!(revoke.header("authorization"), [b"Bearer fixture-checkout"]);
    });
}

fn scope(expires: chrono::DateTime<chrono::Utc>) -> Value {
    json!({
        "token": CHECKOUT_TOKEN,
        "expires_at": expires,
        "permissions": {"contents": "read", "metadata": "read"},
        "repository_selection": "selected",
        "repositories": [{"id": 127, "full_name": "octo/widgets"}]
    })
}
