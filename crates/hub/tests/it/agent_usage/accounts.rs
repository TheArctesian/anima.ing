//! The accounts of the AI agent programs (K8 Section 4 `agentUsage.accounts`, Section 8.2
//! `accountSalt`; AIS-022, THR-149): the hub answers with one salt, keeps each
//! device's newest report, and puts the devices of one key under one account. The address is
//! shown to the person, in no log line, and goes with its device.

use std::io;
use std::sync::{Arc, Mutex};

use anima_hub::fleet::events::process_log;
use axum::http::StatusCode;
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::Pusher;
use crate::daemon::batches::cpu_batch;
use crate::daemon::harness::{DOCKER_HOST, Daemons, PVE_01, pushing_now};

pub(super) const SHARED: &str = "3f9a0c55e1d24b7aa0b1c2d3e4f5061728394a5b6c7d8e9f00112233445566ff";
const OWN: &str = "aa9a0c55e1d24b7aa0b1c2d3e4f5061728394a5b6c7d8e9f00112233445566ff";
pub(super) const ADDRESS: &str = "person-9b2e@example.com";

pub(super) const ACCOUNTS: &str = "query { agentUsage { accounts {
    key shortKey source address plan reportedAt devices { name }
    limits { window usedPercent resetsAt reached } } } }";

pub(super) fn account(key: &str, used: u8) -> Value {
    json!({ "source": "codex", "key": key, "address": ADDRESS, "plan": "Pro",
            "limits": [{ "window": "five_hour", "usedPercent": used, "reached": false }] })
}

/// Pushes `accounts` from the device of `pusher`, `step` batches after its first; the status
/// and the body of the answer.
pub(super) async fn push(pusher: &Pusher<'_>, step: i64, accounts: Value) -> (StatusCode, String) {
    // Each report is later than the one before, as on a hub whose clock runs.
    pusher.daemons.clock.advance(TimeDelta::seconds(1));
    let at = pusher.now - TimeDelta::seconds(600 - 10 * step);
    let mut batch = cpu_batch(pusher.device, at, 5.0);
    batch["agentUsage"] = json!({ "state": "reading", "sessionDays": [], "accounts": accounts });
    pusher.daemons.push(&pusher.token, &batch).await
}

fn salt_of(body: &str) -> String {
    let answer: Value = serde_json::from_str(body).unwrap();
    answer["accountSalt"].as_str().unwrap().to_owned()
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn ais_022_each_device_of_a_hub_gets_the_same_salt(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let first = Pusher::new(&daemons, PVE_01, now).await;
    let second = Pusher::new(&daemons, DOCKER_HOST, now).await;

    let (status, body) = push(&first, 0, json!([])).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "an accounts member asks for the salt"
    );
    let salt = salt_of(&body);
    assert_eq!(salt.len(), 44, "32 bytes in standard base64");
    assert_eq!(salt_of(&push(&second, 1, json!([])).await.1), salt);
    assert_eq!(
        salt_of(&push(&first, 2, json!([])).await.1),
        salt,
        "made once"
    );
    assert_eq!(
        first.push("reading", json!([])).await,
        StatusCode::NO_CONTENT,
        "a batch with no accounts member gets no salt"
    );
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn ais_022_the_devices_of_one_key_are_one_account_with_the_newest_limits(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let first = Pusher::new(&daemons, PVE_01, now).await;
    let second = Pusher::new(&daemons, DOCKER_HOST, now).await;

    assert_eq!(
        push(&first, 0, json!([account(SHARED, 20)])).await.0,
        StatusCode::OK
    );
    assert_eq!(
        push(&second, 1, json!([account(SHARED, 35)])).await.0,
        StatusCode::OK
    );
    let answer = daemons.operate(ACCOUNTS, json!({})).await;
    let accounts = answer["data"]["agentUsage"]["accounts"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(accounts.len(), 1, "{answer}");
    assert_eq!(accounts[0]["shortKey"], "3f9a0c55");
    assert_eq!(
        accounts[0]["address"], ADDRESS,
        "the person sees the address"
    );
    assert_eq!(accounts[0]["plan"], "Pro");
    assert_eq!(
        accounts[0]["limits"][0]["usedPercent"], 35,
        "the newest report"
    );
    assert_eq!(accounts[0]["devices"].as_array().unwrap().len(), 2);

    assert_eq!(
        push(&second, 2, json!([account(OWN, 5)])).await.0,
        StatusCode::OK
    );
    let answer = daemons.operate(ACCOUNTS, json!({})).await;
    let accounts = answer["data"]["agentUsage"]["accounts"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(
        accounts.len(),
        2,
        "the device moved to its own account: {answer}"
    );
    assert_eq!(accounts[0]["key"], OWN, "the newest report first");
    assert_eq!(accounts[1]["devices"].as_array().unwrap().len(), 1);
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn ais_022_an_account_that_could_name_a_person_is_refused(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let pusher = Pusher::new(&daemons, PVE_01, now).await;
    let named = json!([{ "source": "codex", "key": "ann@example.com", "limits": [] }]);
    assert_eq!(push(&pusher, 0, named).await.0, StatusCode::BAD_REQUEST);
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn thr_149_an_address_that_is_not_one_is_refused(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let pusher = Pusher::new(&daemons, PVE_01, now).await;
    let mut bad = account(SHARED, 20);
    bad["address"] = json!("person 9b2e at example.com");
    assert_eq!(
        push(&pusher, 0, json!([bad])).await.0,
        StatusCode::BAD_REQUEST
    );
}

/// The address is personal data: deleting the device deletes it (DEV-015).
#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn thr_149_deleting_the_device_deletes_its_address(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let pusher = Pusher::new(&daemons, PVE_01, now).await;
    assert_eq!(
        push(&pusher, 0, json!([account(SHARED, 20)])).await.0,
        StatusCode::OK
    );
    let stored = || async {
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM agent_usage.accounts WHERE address IS NOT NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    };
    assert_eq!(stored().await, 1);
    let deleted = daemons
        .operate(
            "mutation($id: ID!) { deleteDevice(id: $id) }",
            json!({ "id": PVE_01 }),
        )
        .await;
    assert_eq!(deleted["data"]["deleteDevice"], PVE_01, "{deleted}");
    assert_eq!(stored().await, 0);
}

/// A present list is the whole set of the device: a source that it no longer names (signed out,
/// or the accounts turned off) loses its report and its address (THR-149).
#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn thr_149_a_list_that_no_longer_names_the_source_deletes_its_address(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let pusher = Pusher::new(&daemons, PVE_01, now).await;
    assert_eq!(
        push(&pusher, 0, json!([account(SHARED, 20)])).await.0,
        StatusCode::OK
    );
    assert_eq!(push(&pusher, 1, json!([])).await.0, StatusCode::OK);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM agent_usage.accounts")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 0);
    let answer = daemons.operate(ACCOUNTS, json!({})).await;
    assert_eq!(
        answer["data"]["agentUsage"]["accounts"],
        json!([]),
        "{answer}"
    );
}

/// Each address that the wire takes is stored: the database checks only its length, so a
/// locale of the database cannot refuse a row and stop the pushes of the device.
#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn thr_149_each_address_that_the_wire_takes_is_stored(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let pusher = Pusher::new(&daemons, PVE_01, now).await;
    let longest = format!("{}@example.com", "a".repeat(254 - "@example.com".len()));
    for (step, address) in ["pérson\u{200b}\u{ad}@exämple.中国", longest.as_str()]
        .into_iter()
        .enumerate()
    {
        let mut one = account(SHARED, 20);
        one["address"] = json!(address);
        let step = i64::try_from(step).unwrap();
        assert_eq!(
            push(&pusher, step, json!([one])).await.0,
            StatusCode::OK,
            "{address}"
        );
        let answer = daemons.operate(ACCOUNTS, json!({})).await;
        assert_eq!(
            answer["data"]["agentUsage"]["accounts"][0]["address"],
            address
        );
    }
}

/// THR-149 point 7: at TRACE, a stored report, a refused one and the query of the page write the
/// address in no log line.
#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn thr_149_the_hub_logs_no_address(pool: PgPool) {
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

    assert_eq!(
        push(&pusher, 0, json!([account(SHARED, 20)])).await.0,
        StatusCode::OK
    );
    let mut bad = account(SHARED, 20);
    bad["address"] = json!("person-9b2e@example.com>");
    assert_eq!(
        push(&pusher, 1, json!([bad])).await.0,
        StatusCode::BAD_REQUEST
    );
    let answer = daemons.operate(ACCOUNTS, json!({})).await;
    assert_eq!(
        answer["data"]["agentUsage"]["accounts"][0]["address"],
        ADDRESS
    );
    tracing::error!("capture control line");

    let text = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    assert!(
        text.contains("capture control line"),
        "the log was captured"
    );
    assert!(!text.contains("person-9b2e"), "an address reached the log");
}

/// Collects everything the process log writes.
#[derive(Clone, Default)]
pub(super) struct Capture(pub(super) Arc<Mutex<Vec<u8>>>);

impl io::Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
