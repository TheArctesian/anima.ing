//! Who may read the address of an account (THR-149): the person's own clients, signed in with a
//! login or a browser token; never an API key, which is made for a program, and never a builder
//! token. And a write of a report that the database refuses puts no address in the log of the
//! hub.

use anima_hub::fleet::events::process_log;
use axum::http::StatusCode;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::accounts::{ACCOUNTS, ADDRESS, Capture, SHARED, account, push};
use super::{Pusher, read_key};
use crate::daemon::harness::{Daemons, PVE_01, pushing_now};

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn thr_149_an_api_key_reads_the_accounts_but_not_their_address(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let pusher = Pusher::new(&daemons, PVE_01, now).await;
    assert_eq!(
        push(&pusher, 0, json!([account(SHARED, 20)])).await.0,
        StatusCode::OK
    );
    let key = read_key(&daemons, now).await;

    let without = "query { agentUsage { accounts { key plan limits { window } } } }";
    let answer = daemons.graphql(Some(&key), without, json!({})).await;
    assert!(answer["errors"].is_null(), "{answer}");
    assert_eq!(answer["data"]["agentUsage"]["accounts"][0]["key"], SHARED);

    let answer = daemons.graphql(Some(&key), ACCOUNTS, json!({})).await;
    assert!(answer["errors"].is_null(), "{answer}");
    assert_eq!(
        answer["data"]["agentUsage"]["accounts"][0]["address"],
        Value::Null
    );
    assert!(!answer.to_string().contains(ADDRESS), "{answer}");

    let answer = daemons.operate(ACCOUNTS, json!({})).await;
    assert_eq!(
        answer["data"]["agentUsage"]["accounts"][0]["address"],
        ADDRESS
    );
}

/// A builder token reaches no field but the builder fields (BLD-022), so the accounts, and
/// their addresses with them, are refused to it as a whole.
#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn thr_149_a_builder_token_gets_no_account_and_no_address(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let pusher = Pusher::new(&daemons, PVE_01, now).await;
    assert_eq!(
        push(&pusher, 0, json!([account(SHARED, 20)])).await.0,
        StatusCode::OK
    );
    let enrol = "mutation { enrolBuilder(name: \"pop-os\", \
                 platform: { os: LINUX, arch: X86_64 }) { token } }";
    let enrolment = daemons.operate(enrol, json!({})).await;
    assert!(enrolment["errors"].is_null(), "{enrolment}");
    let builder = enrolment["data"]["enrolBuilder"]["token"].as_str().unwrap();

    let answer = daemons.graphql(Some(builder), ACCOUNTS, json!({})).await;
    assert_eq!(
        answer["errors"][0]["extensions"]["code"], "FORBIDDEN",
        "{answer}"
    );
    assert!(answer["data"]["agentUsage"].is_null(), "{answer}");
    assert!(!answer.to_string().contains(ADDRESS), "{answer}");
}

/// A write that the database refuses (here a check added for the test) makes the push
/// `UNAVAILABLE` and writes the address in no log line of the hub (THR-149 point 7).
#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn thr_149_a_refused_write_logs_no_address(pool: PgPool) {
    sqlx::query(
        "ALTER TABLE agent_usage.accounts ADD CONSTRAINT test_refuses \
         CHECK (address NOT LIKE 'person-%')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let capture = Capture::default();
    let writer = capture.clone();
    let log = process_log::subscriber("trace", move || writer.clone()).unwrap();
    // As in bearer::metrics: a second dispatcher keeps another test thread from caching this
    // test's callsites as disabled.
    let _second = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let _default = tracing::subscriber::set_default(log);
    tracing::callsite::rebuild_interest_cache();
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let pusher = Pusher::new(&daemons, PVE_01, now).await;

    let (status, _) = push(&pusher, 0, json!([account(SHARED, 20)])).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    tracing::error!("capture control line");

    let text = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    assert!(
        text.contains("capture control line"),
        "the log was captured"
    );
    assert!(
        text.contains("the agent usage cannot be stored"),
        "the refused write was logged: {text}"
    );
    assert!(!text.contains("person-9b2e"), "an address reached the log");
}
