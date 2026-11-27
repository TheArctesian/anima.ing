//! Only a matching Compose inventory entry on the application-enabled node is marked.
use std::sync::Arc;

use async_graphql::{EmptyMutation, EmptySubscription, Request, Schema};
use serde_json::{Value, json};
use sqlx::PgPool;

use crate::{
    clock::{Clock, FixedClock},
    fleet::{
        access::Identity,
        events::{Actor, Operator},
        ssh::tests::fixture::now,
    },
    ids::DeviceId,
    work::containers::{
        graphql::Query,
        tests::{ID, LISTED, device, exiting, policy},
    },
};

const APP: &str = "0190a000-0000-7000-8002-0000000000ab";
const PROJECT: &str = "anima-0190a0000000700080020000000000ab";
const OTHER_NODE: &str = "0190a000-0000-7000-8002-000000000001";

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../../../../tests/fixtures", scripts("devices"))
)]
async fn hosting_marks_only_the_completed_apply_on_the_observed_device(pool: PgPool) {
    let bench = device(&pool, exiting(&docker_inventory(), 0)).await;
    let schema = Schema::build(Query, EmptyMutation, EmptySubscription)
        .data(pool.clone())
        .data(bench.vault)
        .data(policy())
        .data(Arc::new(FixedClock(now())) as Arc<dyn Clock>)
        .finish();
    let request = || {
        let operator = Operator {
            id: "0190a000-0000-7000-8000-000000000001".parse().unwrap(),
            name: "operator".to_owned(),
            actor: Actor::for_tests(),
        };
        let query = format!(
            r#"{{ containers(deviceId:"{}") {{ name managedApplicationId }} }}"#,
            bench.device.id
        );
        Identity::for_tests(operator, now() + chrono::TimeDelta::days(1))
            .attach(Request::new(query))
    };
    let observed = async || {
        let response = schema.execute(request()).await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        response.data.into_json().unwrap()["containers"].clone()
    };

    let other = OTHER_NODE.parse().unwrap();
    completed_apply(&pool, other).await;
    let foreign = observed().await;
    assert!(foreign.as_array().unwrap().iter().all(unmanaged));

    completed_apply(&pool, bench.device.id).await;
    assert_eq!(
        observed().await,
        json!([
            {"name": "dawarich_app", "managedApplicationId": APP},
            {"name": "other-folder", "managedApplicationId": null}
        ])
    );
}

fn unmanaged(container: &Value) -> bool {
    container["managedApplicationId"].is_null()
}

fn docker_inventory() -> String {
    let folder = format!("/persist/anima-applications/{APP}/files");
    let owned = LISTED
        .lines()
        .next()
        .unwrap()
        .replace(
            r#""project":"dawarich""#,
            &format!(r#""project":"{PROJECT}""#),
        )
        .replace(
            "/srv/dawarich/compose.yaml,/srv/dawarich/override.yaml",
            &format!("{folder}/final-form.json"),
        )
        .replace(r#""dir":"/srv/dawarich""#, &format!(r#""dir":"{folder}""#));
    let other = owned
        .replace(r#""names":"dawarich_app""#, r#""names":"other-folder""#)
        .replace(ID, &"b".repeat(64))
        .replace(&folder, "/srv/other");
    format!("{owned}\n{other}\n")
}

async fn completed_apply(pool: &PgPool, device: DeviceId) {
    sqlx::query(
        "INSERT INTO jobs.application_machines (application_id, device_id)
         VALUES ($1, $2) ON CONFLICT (application_id) DO UPDATE SET device_id = EXCLUDED.device_id",
    )
    .bind(APP.parse::<uuid::Uuid>().unwrap())
    .bind(device)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO jobs.jobs (id, work_type, kind, machine_id, parameters, secrets,
           limit_seconds, limit_idle_seconds, limit_memory_bytes, limit_disk_bytes,
           limit_orphan_seconds, state, attempt, created_at, state_changed_at, ended_at)
         VALUES ($1, 'application_apply', 'host', $2, $3, array['compose_file'],
           60, 30, 1, 1, 30, 'done', 1, $4, $4, $4)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(device)
    .bind(json!({"application": APP}))
    .bind(now())
    .execute(pool)
    .await
    .unwrap();
}
