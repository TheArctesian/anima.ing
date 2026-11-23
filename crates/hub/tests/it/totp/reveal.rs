//! A sealed record's reveal asks the second factor again after 5 minutes (K-E5 Section 3; VLT-004, VLT-005).

use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

use anima_hub::{
    clock::{Clock, FixedClock},
    config,
    fleet::{access::RECHECK_WINDOW, events::Bus},
    schema::Dependencies,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use chrono::{DateTime, TimeDelta, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::{code, enrol, login};
use crate::hub::{Hub, PASSWORD, dependencies, error, set_operator, start};

const OWNER: &str = "passport:0190a000-0000-7000-8000-00000000bbbb";
const SEALED: &[u8] = b"bytes that a client sealed";
const REVEAL: &str = "mutation($o: String!, $c: String) { recordReveal(owner: $o, totpCode: $c) }";

async fn store(hub: &Hub, token: &str) {
    let input = json!({ "owner": OWNER, "extension": "documents",
                        "sealed": STANDARD.encode(SEALED), "keyId": STANDARD.encode([7_u8; 32]) });
    let stored = hub
        .post(
            Some(token),
            "mutation($i: SealedRecordInput!) { storeSealedRecord(input: $i) { id } }",
            json!({ "i": input }),
        )
        .await;
    assert!(stored["errors"].is_null(), "{stored}");
}

async fn reveal(hub: &Hub, token: &str, totp_code: Option<&str>) -> Value {
    hub.post(Some(token), REVEAL, json!({ "o": OWNER, "c": totp_code }))
        .await
}

fn revealed(response: &Value) -> bool {
    response["data"]["recordReveal"] == true
}

fn reason(response: &Value) -> &Value {
    &response["errors"][0]["extensions"]["reason"]
}

fn wrong_code(secret: &[u8], at: DateTime<Utc>) -> String {
    let right: u32 = code(secret, at).parse().unwrap();
    format!("{:06}", (right + 1) % 1_000_000)
}

async fn signed_in_with_code(pool: &PgPool, secret: &[u8]) -> (Hub, String) {
    let hub = Hub::new(pool, start());
    let signed = login(&hub, PASSWORD, Some(&code(secret, start()))).await;
    let token = signed["data"]["login"]["token"]
        .as_str()
        .unwrap()
        .to_owned();
    (hub, token)
}

#[sqlx::test(migrations = "./migrations")]
async fn vlt_005_a_reveal_takes_the_code_of_the_sign_in_for_5_minutes_then_a_new_one(pool: PgPool) {
    let secret = enrol(&pool).await;
    let (hub, token) = signed_in_with_code(&pool, &secret).await;
    store(&hub, &token).await;
    let first = reveal(&hub, &token, None).await;
    assert!(revealed(&first), "{first}");

    let later = start() + RECHECK_WINDOW + TimeDelta::seconds(1);
    let late = Hub::new(&pool, later);
    let refused = reveal(&late, &token, None).await;
    assert_eq!(error(&refused).1, "FORBIDDEN", "{refused}");
    assert_eq!(reason(&refused), "TOTP_REQUIRED");
    let fresh = code(&secret, later);
    assert!(revealed(&reveal(&late, &token, Some(&fresh)).await));
    assert!(revealed(&reveal(&late, &token, None).await));

    let entries: Vec<Value> = sqlx::query_scalar(
        "select change from events.audit_log where target_type = 'sealed_record' \
         and action = 'reveal' order by at, id",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(entries.len(), 3, "{entries:?}");
    assert_eq!(entries[0]["owner"], OWNER);
    let written = format!("{entries:?}");
    assert!(!written.contains(&STANDARD.encode(SEALED)), "{written}");
    assert!(!written.contains(&fresh), "{written}");
}

#[sqlx::test(migrations = "./migrations")]
async fn vlt_005_five_wrong_codes_sign_the_login_out(pool: PgPool) {
    let secret = enrol(&pool).await;
    let (hub, token) = signed_in_with_code(&pool, &secret).await;
    store(&hub, &token).await;
    let later = start() + RECHECK_WINDOW + TimeDelta::seconds(1);
    let late = Hub::new(&pool, later);
    let wrong = wrong_code(&secret, later);
    for _ in 0..4 {
        let refused = reveal(&late, &token, Some(&wrong)).await;
        assert_eq!(reason(&refused), "TOTP_REQUIRED", "{refused}");
    }
    let fifth = reveal(&late, &token, Some(&wrong)).await;
    assert_eq!(error(&fifth).1, "UNAUTHENTICATED", "{fifth}");
    let me = late
        .post(Some(&token), "{ me { username } }", json!({}))
        .await;
    assert_eq!(error(&me).1, "UNAUTHENTICATED", "{me}");
    let warned: i64 = sqlx::query_scalar(
        "select count(*) from events.event_log where message like '%wrong TOTP codes%'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(warned, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn vlt_005_a_reveal_is_refused_with_no_totp_with_an_api_key_and_on_a_hosted_hub(
    pool: PgPool,
) {
    set_operator(&pool).await;
    let hub = Hub::new(&pool, start());
    let token = hub.token().await;
    store(&hub, &token).await;
    let no_totp = reveal(&hub, &token, Some("123456")).await;
    assert_eq!(error(&no_totp).1, "FORBIDDEN", "{no_totp}");
    assert_eq!(reason(&no_totp), "TOTP_NOT_ENROLLED");
    let unknown = hub
        .post(
            Some(&token),
            REVEAL,
            json!({ "o": "passport:0190a000-0000-7000-8000-00000000cccc" }),
        )
        .await;
    assert_eq!(error(&unknown).1, "NOT_FOUND", "{unknown}");

    let input = json!({ "name": "script", "scope": "WRITE", "madeFor": "USER",
                        "expiresAt": start() + TimeDelta::days(30) });
    let key = hub
        .post(
            Some(&token),
            "mutation($i: ApiKeyInput!) { createApiKey(input: $i) { secret } }",
            json!({ "i": input }),
        )
        .await;
    let key = key["data"]["createApiKey"]["secret"]
        .as_str()
        .unwrap_or_else(|| panic!("{key}"));
    let by_key = reveal(&hub, key, None).await;
    assert_eq!(error(&by_key).1, "FORBIDDEN", "{by_key}");

    let mut hosted = config::from_env(&BTreeMap::new()).unwrap();
    hosted.hosted_cloud_key_file = Some(PathBuf::from("/nonexistent/cloud_key"));
    let clock: Arc<dyn Clock> = Arc::new(FixedClock(start()));
    let parts = Dependencies {
        config: Arc::new(hosted),
        ..dependencies(&pool, clock, Bus::new(16))
    };
    let on_hosted = reveal(&Hub::serving(&pool, parts), &token, None).await;
    assert_eq!(error(&on_hosted).1, "UNAVAILABLE", "{on_hosted}");
    let reveals: i64 =
        sqlx::query_scalar("select count(*) from events.audit_log where action = 'reveal'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(reveals, 0);
}
