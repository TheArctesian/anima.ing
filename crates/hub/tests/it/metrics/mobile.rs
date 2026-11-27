//! THR-172: K8 replacement and the typed K1 read boundary share the stored snapshot.
use crate::daemon::{
    batches::cpu_batch,
    harness::{Daemons, PVE_01, pushing_now},
};
use axum::http::StatusCode;
use chrono::TimeDelta;
use serde_json::{Value, json};
use sqlx::PgPool;

const QUERY: &str = "{ capabilities { mobileTools { receivedAt apple { simulatorRuntimes { state entries { identifier version available } } deviceTypes { state entries { identifier } } } android { sdkPackages { state entries { path revision } } emulator { state version } } } } }";

fn mobile() -> Value {
    json!({"apple":{"simulatorRuntimes":{"state":"read","entries":[]},"deviceTypes":{"state":"missing"}},"android":{"sdkPackages":{"state":"read","entries":[]},"emulator":{"state":"read","version":"35.2.10"}}})
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn mobile_inventory_replaces_failures_and_clears_on_daemon_downgrade(pool: PgPool) {
    let now = pushing_now(&pool).await;
    let daemons = Daemons::at(&pool, now).await;
    let token = daemons.token_for(PVE_01).await;
    let mut batch = cpu_batch(PVE_01, now - TimeDelta::seconds(30), 5.0);
    batch["inventory"] = json!({"architecture":"aarch64","os":"macos","osVersion":"27.0","tools":[],"buildTargets":[],"mobileTools":mobile()});
    assert_eq!(daemons.push(&token, &batch).await.0, StatusCode::NO_CONTENT);
    let read = daemons.operate(QUERY, json!({})).await;
    let first = &read["data"]["capabilities"][0]["mobileTools"];
    assert_eq!(first["android"]["emulator"]["version"], "35.2.10");
    assert_eq!(first["apple"]["simulatorRuntimes"]["entries"], json!([]));
    assert_eq!(first["apple"]["deviceTypes"]["entries"], Value::Null);
    assert_eq!(
        chrono::DateTime::parse_from_rfc3339(first["receivedAt"].as_str().unwrap())
            .unwrap()
            .with_timezone(&chrono::Utc),
        now
    );
    batch["inventory"]["mobileTools"]["android"]["emulator"] = json!({"state":"timed_out"});
    assert_eq!(daemons.push(&token, &batch).await.0, StatusCode::NO_CONTENT);
    let failed = daemons.operate(QUERY, json!({})).await;
    assert_eq!(
        failed["data"]["capabilities"][0]["mobileTools"]["android"]["emulator"],
        json!({"state":"TIMED_OUT","version":null})
    );
    let no_inventory = cpu_batch(PVE_01, now - TimeDelta::seconds(20), 5.0);
    assert_eq!(
        daemons.push(&token, &no_inventory).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(daemons.operate(QUERY, json!({})).await, failed);
    batch["inventory"]
        .as_object_mut()
        .unwrap()
        .remove("mobileTools");
    assert_eq!(daemons.push(&token, &batch).await.0, StatusCode::NO_CONTENT);
    assert_eq!(
        daemons.operate(QUERY, json!({})).await["data"]["capabilities"][0]["mobileTools"],
        Value::Null
    );
}
