//! The hand-out of model items through the work route (K11 Section 3; compute plan task L1-5):
//! a node whose device policy stops it gets no model item, but each reducing one.

use anima_hub::work::jobs::{self, NewJob, Target};
use axum::http::StatusCode;
use chrono::TimeDelta;
use daemon_core::{
    model_work::names::Actor,
    wire::{MODEL_TYPES, WorkKind, WorkType},
};
use serde_json::{Value, json};

use super::{Work, build, claim_of};
use crate::daemon::harness::PVE_01;

const SET: &str = "mutation($deviceId: ID!, $input: DevicePolicyInput!) {
    setDevicePolicy(deviceId: $deviceId, input: $input) { takesModelWork stopReason } }";

fn model(work_type: WorkType, parameters: Value) -> NewJob {
    NewJob {
        work_type,
        kind: WorkKind::Model,
        os: None,
        architecture: None,
        target: Target::Model { actor: Actor::Hub },
        parameters,
        ..build()
    }
}

/// The claim of a Linux machine that takes model items and holds `holds`.
pub(super) fn model_claim(holds: &Value) -> Value {
    let mut claim = claim_of(PVE_01, holds);
    claim["kinds"] = json!(["container", "model"]);
    let mut types = vec![json!("build"), json!("runner")];
    types.extend(MODEL_TYPES.iter().map(|each| json!(each)));
    claim["types"] = json!(types);
    claim
}

/// Sets the policy of the machine: quiet hours around the time of the test, or none.
async fn set_quiet(work: &Work, quiet: bool) -> Value {
    let hours = quiet.then(|| {
        let at = |delta: TimeDelta| (work.now() + delta).format("%H:%M").to_string();
        json!({ "start": at(TimeDelta::hours(-1)), "end": at(TimeDelta::hours(1)), "timeZone": "UTC" })
    });
    let input = json!({ "memorySharePercent": 100, "diskSharePercent": 75, "quietHours": hours,
        "mainsOnly": true, "idleOnly": false });
    let variables = json!({ "deviceId": PVE_01, "input": input });
    work.daemons.operate(SET, variables).await["data"]["setDevicePolicy"].clone()
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn cap_010_a_node_whose_policy_stops_it_gets_no_model_item_but_a_drain(pool: sqlx::PgPool) {
    let work = Work::new(&pool).await;
    let token = work.daemons.token_for(PVE_01).await;
    let (status, _) = work.claim(&token, &model_claim(&json!([]))).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "the hub learns the switch");
    let set = set_quiet(&work, true).await;
    assert_eq!(set["stopReason"], "QUIET_HOURS", "{set}");

    let start = work
        .enqueue(model(WorkType::ServerStart, json!({ "copy": "x" })))
        .await;
    let (status, body) = work.claim(&token, &model_claim(&json!([]))).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let reason = jobs::placement_reason(work.queue(), start.id)
        .await
        .unwrap();
    assert!(
        reason.is_some_and(|line| line.ends_with("its device policy takes no model work now")),
        "the decision names the policy"
    );

    let drain = work
        .enqueue(model(WorkType::ModelDrain, json!({ "seconds": 60 })))
        .await;
    let (status, item) = work.claim(&token, &model_claim(&json!([]))).await;
    assert_eq!(status, StatusCode::OK, "{item}");
    assert_eq!(item["id"], json!(drain.id.to_string()));
    assert_eq!(item["type"], "model_drain");
    assert_eq!(item["kind"], "model");
    assert_eq!(item["actor"], "Hub");
    assert_eq!(item["project"], Value::Null);
    assert!(
        item.get("repository").is_none(),
        "a model item has no repository: {item}"
    );
    let members: Vec<&String> = item.as_object().unwrap().keys().collect();
    assert_eq!(
        members.len(),
        10,
        "the members of K11 Section 2: {members:?}"
    );

    let set = set_quiet(&work, false).await;
    assert_eq!(set["takesModelWork"], true, "{set}");
    let holds = json!([{ "id": item["id"], "attempt": item["attempt"] }]);
    let (status, item) = work.claim(&token, &model_claim(&holds)).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a held drain counts for nothing: {item}"
    );
    assert_eq!(item["id"], json!(start.id.to_string()));
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn a_model_item_goes_before_a_job_and_takes_no_slot(pool: sqlx::PgPool) {
    let work = Work::new(&pool).await;
    let token = work.daemons.token_for(PVE_01).await;
    let job = work.enqueue(build()).await;
    let report = work.enqueue(model(WorkType::ModelReport, json!({}))).await;

    let (status, item) = work.claim(&token, &model_claim(&json!([]))).await;
    assert_eq!(status, StatusCode::OK, "{item}");
    assert_eq!(item["id"], json!(report.id.to_string()));

    let holds = json!([{ "id": item["id"], "attempt": item["attempt"] }]);
    let (status, item) = work.claim(&token, &model_claim(&holds)).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a held model item leaves the slot: {item}"
    );
    assert_eq!(item["id"], json!(job.id.to_string()));
    assert!(item.get("actor").is_none(), "a job has no actor: {item}");
}

#[sqlx::test(
    migrations = "./migrations",
    fixtures(path = "../../fixtures", scripts("devices"))
)]
async fn a_model_item_is_refused_unless_it_is_whole(pool: sqlx::PgPool) {
    let work = Work::new(&pool).await;
    let asker = (
        anima_hub::fleet::events::Operator::hub(),
        anima_hub::fleet::events::ClientAddr(crate::daemon::harness::PERMITTED.parse().unwrap()),
    );
    let half = [
        NewJob {
            kind: WorkKind::Container,
            ..model(WorkType::ServerStart, json!({}))
        },
        NewJob {
            work_type: WorkType::Build,
            ..model(WorkType::ServerStart, json!({}))
        },
        NewJob {
            target: build().target,
            ..model(WorkType::ServerStart, json!({}))
        },
        NewJob {
            secrets: vec!["hf_token".to_owned()],
            ..model(WorkType::ServerStart, json!({}))
        },
    ];
    for wanted in half {
        let refused = jobs::enqueue(work.queue(), wanted.clone(), asker.clone(), work.now()).await;
        assert!(
            matches!(refused, Err(jobs::Error::Invalid(_))),
            "{wanted:?}: {refused:?}"
        );
    }
    let gated = NewJob {
        secrets: vec!["hf_token".to_owned()],
        ..model(WorkType::ModelPull, json!({}))
    };
    assert!(
        jobs::enqueue(work.queue(), gated, asker, work.now())
            .await
            .is_ok()
    );
}
