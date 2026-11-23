//! Daemon token renewal (MET-013, K8 Section 5).

use axum::http::StatusCode;
use chrono::{DateTime, TimeDelta, Utc};
use daemon_core::wire::DaemonToken;
use serde_json::Value;
use sqlx::PgPool;

use super::{
    batches::{cpu_batch, samples_of},
    harness::{Daemons, PERMITTED, PVE_01, pushing_now, refusal},
};
use crate::hub::start;

fn refused() -> (StatusCode, String) {
    (StatusCode::UNAUTHORIZED, refusal("TOKEN_REFUSED"))
}

fn never_issued(seed: u8) -> String {
    DaemonToken::from_random([seed; 32]).expose().to_owned()
}

/// The write checks the hash again, so only a malformed body shows a bearer that let `token` through.
async fn refused_at_the_bearer(daemons: &Daemons, token: &str) {
    let bearer = format!("Bearer {token}");
    let pushed = daemons.push_raw(Some(&bearer), "{}", PERMITTED).await;
    assert_eq!(pushed, refused());
    let renewal = daemons.renew_raw(Some(&bearer), "[]", PERMITTED).await;
    assert_eq!(renewal, refused());
}

async fn daemon_rows(pool: &PgPool) -> (String, i64) {
    let row = sqlx::query_scalar(
        "SELECT row_to_json(a)::text FROM metrics.daemons a WHERE device_id = $1::uuid",
    )
    .bind(PVE_01)
    .fetch_one(pool)
    .await
    .unwrap();
    (row, samples_of(pool, PVE_01).await)
}

/// T8 S3.
#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn met_013_a_token_never_issued_is_refused_while_the_device_is_enrolled(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let token = daemons.token_for(PVE_01).await;
    let before = daemon_rows(&pool).await;
    let stranger = never_issued(0xa5);
    assert_ne!(stranger, token);
    let batch = cpu_batch(PVE_01, now, 12.5);
    assert_eq!(daemons.push(&stranger, &batch).await, refused());
    assert_eq!(daemons.renew(&stranger).await, refused());
    refused_at_the_bearer(&daemons, &stranger).await;
    assert_eq!(daemon_rows(&pool).await, before);
    assert_eq!(daemons.push(&token, &batch).await.0, StatusCode::NO_CONTENT);
}

/// T8 S3.
#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn met_013_a_token_neither_current_nor_renewed_is_refused_while_a_renewal_waits(
    pool: PgPool,
) {
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let current = daemons.token_for(PVE_01).await;
    let renewed = daemons.renewed(&current).await;
    let before = daemon_rows(&pool).await;
    let stranger = never_issued(0x5a);
    assert!(stranger != current && stranger != renewed);
    let batch = cpu_batch(PVE_01, now, 12.5);
    assert_eq!(daemons.push(&stranger, &batch).await, refused());
    assert_eq!(daemons.renew(&stranger).await, refused());
    refused_at_the_bearer(&daemons, &stranger).await;
    assert_eq!(
        daemon_rows(&pool).await,
        before,
        "nothing promoted or stored"
    );
    assert_eq!(
        daemons.push(&renewed, &batch).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        daemons.renew(&current).await,
        refused(),
        "promoted by its first use"
    );
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn met_013_the_old_token_works_until_the_new_one_is_first_used(pool: PgPool) {
    let daemons = Daemons::new(&pool).await;
    let first = daemons.token_for(PVE_01).await;
    let unused = daemons.renewed(&first).await;
    let second = daemons.renewed(&first).await;
    assert_eq!(daemons.renew(&unused).await, refused());
    let third = daemons.renewed(&second).await;
    assert_eq!(daemons.renew(&first).await, refused());
    let fourth = daemons.renewed(&second).await;
    assert_eq!(daemons.renew(&third).await, refused());
    assert_eq!(daemons.renew(&fourth).await.0, StatusCode::OK);
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn met_013_a_token_works_for_365_days(pool: PgPool) {
    let daemons = Daemons::new(&pool).await;
    let token = daemons.token_for(PVE_01).await;
    daemons
        .clock
        .advance(TimeDelta::days(365) - TimeDelta::seconds(1));
    let (status, text) = daemons.renew(&token).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let renewed: Value = serde_json::from_str(&text).unwrap();
    let expires_at: DateTime<Utc> = renewed["expiresAt"].as_str().unwrap().parse().unwrap();
    assert_eq!(
        expires_at,
        start() + TimeDelta::days(730) - TimeDelta::seconds(1)
    );
    let next = renewed["token"].as_str().unwrap().to_owned();

    daemons.clock.advance(TimeDelta::seconds(1));
    assert_eq!(daemons.renew(&token).await, refused());
    let newest = daemons.renewed(&next).await;
    daemons.clock.advance(TimeDelta::days(365));
    assert_eq!(daemons.renew(&newest).await, refused());
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn k8_renewal_checks_the_bearer_then_the_body(pool: PgPool) {
    let daemons = Daemons::new(&pool).await;
    let token = daemons.token_for(PVE_01).await;
    let bearer = format!("Bearer {token}");
    let malformed = (StatusCode::BAD_REQUEST, refusal("MALFORMED"));
    for body in ["", "[]", "null", "\"{}\"", "{", "{} {}"] {
        let answer = daemons.renew_raw(Some(&bearer), body, PERMITTED).await;
        assert_eq!(answer, malformed, "{body}");
    }
    let large = format!(r#"{{"pad":"{}"}}"#, "x".repeat(4096));
    let answer = daemons
        .renew_raw(Some(&bearer), large.clone(), PERMITTED)
        .await;
    assert_eq!(
        answer,
        (StatusCode::PAYLOAD_TOO_LARGE, refusal("TOO_LARGE"))
    );
    for authorization in [None, Some("Bearer junk"), Some(&*format!("Basic {token}"))] {
        let answer = daemons
            .renew_raw(authorization, large.clone(), PERMITTED)
            .await;
        assert_eq!(answer, refused(), "{authorization:?}");
    }
    let extra = daemons
        .renew_raw(Some(&bearer), r#"{"daemonVersion":"0.1.0"}"#, PERMITTED)
        .await;
    assert_eq!(extra.0, StatusCode::OK, "{}", extra.1);
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn met_013_two_first_uses_of_a_renewed_token_at_once_are_neither_refused(pool: PgPool) {
    let daemons = Daemons::new(&pool).await;
    let first = daemons.token_for(PVE_01).await;
    let renewed = daemons.renewed(&first).await;
    let (one, other) = tokio::join!(daemons.renew(&renewed), daemons.renew(&renewed));
    // The device's one turn (MET-017) may answer the later one `UNAVAILABLE`, never `TOKEN_REFUSED`.
    let unavailable = (StatusCode::SERVICE_UNAVAILABLE, refusal("UNAVAILABLE"));
    for answer in [&one, &other] {
        assert!(
            answer.0 == StatusCode::OK || *answer == unavailable,
            "{answer:?}"
        );
    }
    assert!(
        one.0 == StatusCode::OK || other.0 == StatusCode::OK,
        "{one:?} {other:?}"
    );
    assert_eq!(daemons.renew(&first).await, refused());
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn met_013_enrolling_again_ends_a_renewed_token_never_used(pool: PgPool) {
    let daemons = Daemons::new(&pool).await;
    let first = daemons.token_for(PVE_01).await;
    let renewed = daemons.renewed(&first).await;
    let again = daemons.token_for(PVE_01).await;
    assert_eq!(daemons.renew(&renewed).await, refused());
    assert_eq!(daemons.renew(&again).await.0, StatusCode::OK);
}
