use super::{
    Phase, Record, Resources, capture_source, mark_stopping, snapshot, start_build, store,
};
use crate::linux::{
    Result,
    builds::{
        self,
        lease::{Grant, Lease},
    },
    source_jobs::{Request, require_build_lease},
    work_policy,
    workspace::context::{self, Identity, tests as fixture},
};
use anima_os_maintenance::{Admission, drain, recovery, source_pipeline::Pipeline};
use chrono::{TimeDelta, Utc};
use serde_json::json;
use std::{fs, path::Path, time::Duration};
use tokio::task::JoinHandle;
use uuid::Uuid;

mod memory;
mod staged;
mod startup;

#[tokio::test]
#[ignore = "requires disposable root Linux with an empty Btrfs /persist and packaged commands"]
async fn source_capture_waits_for_pressure_and_honors_the_lease() {
    let identity = fixture::prepare_workspace().await;
    let pipeline = accepted_pipeline(identity.workspace);
    let execution = pipeline.execution();
    drain::begin().unwrap();

    let hold = Path::new(fixture::PRESSURE).join("active");
    fs::write(&hold, b"synthetic hosting pressure").unwrap();
    let mut worker = waiting_capture(pipeline, identity, TimeDelta::seconds(30)).await;
    fs::remove_file(Path::new(fixture::PRESSURE).join("clear")).unwrap();
    fs::remove_file(&hold).unwrap();
    let stale = tokio::time::timeout(Duration::from_millis(1100), &mut worker).await;
    assert!(
        stale.is_err(),
        "capture returned without a current pressure sample: {stale:?}"
    );
    fixture::clear_sample();
    finish(worker).await.unwrap();
    let context = context::status(identity.workspace, 1, identity.context).unwrap();
    assert_eq!(context["phase"], "captured");
    assert_eq!(context["entries"], 1);

    fs::write(&hold, b"synthetic hosting pressure").unwrap();
    let cancelled = Identity {
        context: Uuid::now_v7(),
        ..identity
    };
    let worker = waiting_capture(pipeline, cancelled, TimeDelta::seconds(30)).await;
    mark_stopping(execution).unwrap();
    assert!(
        finish(worker)
            .await
            .unwrap_err()
            .to_string()
            .contains("no longer authorized")
    );
    assert!(context::status(cancelled.workspace, 1, cancelled.context).is_err());

    let expired = Identity {
        context: Uuid::now_v7(),
        ..identity
    };
    let worker = waiting_capture(pipeline, expired, TimeDelta::milliseconds(900)).await;
    assert!(
        finish(worker)
            .await
            .unwrap_err()
            .to_string()
            .contains("no longer authorized")
    );
    assert_eq!(
        snapshot(execution).unwrap().failure.as_deref(),
        Some("lease_expired")
    );
    assert!(context::status(expired.workspace, 1, expired.context).is_err());
}

async fn waiting_capture(
    pipeline: Pipeline,
    identity: Identity,
    span: TimeDelta,
) -> JoinHandle<Result<()>> {
    let execution = pipeline.execution();
    save_fixture(&checked_out_job(pipeline, identity, span));
    let mut worker = tokio::spawn(capture_source(execution, pipeline));
    let early = tokio::time::timeout(Duration::from_millis(150), &mut worker).await;
    assert!(
        early.is_err(),
        "capture returned during a temporary hold: {early:?}"
    );
    assert!(snapshot(execution).unwrap().phase == Phase::Capturing);
    assert!(context::status(identity.workspace, 1, identity.context).is_err());
    drop(Admission::cleanup().expect("a waiting capture must leave admission free"));
    worker
}

async fn finish<T>(worker: JoinHandle<Result<T>>) -> Result<T> {
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
#[ignore = "requires disposable root Linux with an empty /persist and packaged commands"]
async fn source_lease_renews_before_the_build_receipt_exists() {
    let identity = fixture::prepare_workspace().await;
    let pipeline = accepted_pipeline(identity.workspace);
    fs::create_dir_all(Path::new(builds::store::ROOT).join("locks")).unwrap();
    work_policy::Request::WorkPolicySet {
        revision: 0,
        enabled: true,
        repositories: "example/source".into(),
    }
    .execute()
    .unwrap();
    let mut record = checked_out_job(pipeline, identity, TimeDelta::milliseconds(900));
    record.phase = Phase::Building;
    save_fixture(&record);
    let hold = Path::new(fixture::PRESSURE).join("active");
    fs::write(&hold, b"synthetic hosting pressure").unwrap();
    let resources = Resources {
        memory_bytes: 512 << 20,
        cpu_millis: 500,
        tasks: 128,
    };
    let mut worker = tokio::spawn(start_build(record.execution, pipeline, resources));
    let early = tokio::time::timeout(Duration::from_millis(150), &mut worker).await;
    assert!(
        early.is_err(),
        "the build returned during a temporary hold: {early:?}"
    );
    let lease = Grant {
        until: Utc::now() + TimeDelta::seconds(60),
        ..record.lease.grant
    };
    let build_preflight = builds::store::lock(record.build).unwrap();
    let renewed = Request::Renew {
        execution: record.execution,
        lease,
    }
    .execute()
    .await
    .unwrap();
    drop(build_preflight);
    assert_eq!(renewed["phase"], "building");
    assert_eq!(
        require_build_lease(pipeline, identity, record.lease.grant)
            .unwrap()
            .grant
            .until,
        lease.until
    );
    let still_waiting = tokio::time::timeout(Duration::from_millis(1100), &mut worker).await;
    assert!(
        still_waiting.is_err(),
        "the original lease expired but its renewal is current: {still_waiting:?}"
    );
    assert!(!builds::store::base(record.build).exists());
    mark_stopping(record.execution).unwrap();
    fs::remove_file(hold).unwrap();
    assert!(require_build_lease(pipeline, identity, lease).is_err());
    let failure = finish(worker).await.unwrap_err();
    assert!(failure.to_string().contains("no longer authorized"));
    assert!(!builds::store::base(record.build).exists());
}

fn save_fixture(record: &Record) {
    let _lock = store::lock().unwrap();
    store::save(record).unwrap();
}

fn accepted_pipeline(workspace: Uuid) -> Pipeline {
    let execution = Uuid::now_v7();
    let admission = Admission::cleanup().unwrap();
    recovery::complete(&admission).unwrap();
    fixture::clear_sample();
    Pipeline::accept(&admission, (execution, workspace, Uuid::now_v7())).unwrap();
    Pipeline::resume(execution).unwrap()
}

fn checked_out_job(pipeline: Pipeline, identity: Identity, span: TimeDelta) -> Record {
    let now = Utc::now();
    let until = now + span;
    let job = Uuid::now_v7();
    Record {
        schema: 1, execution: pipeline.execution(), device: Uuid::now_v7(), origin: "https://example.invalid".into(),
        job: serde_json::from_value(json!({
            "id":job,"type":"build","kind":"container","attempt":1,
            "lease":{"until":until.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)},
            "limits":{"seconds":60,"idleSeconds":15,"memoryBytes":536_870_912,"diskBytes":22_548_578_304_u64,"orphanSeconds":10},
            "project":null,"repository":{"forge":"github","owner":"example","name":"source"},
            "secrets":[],"parameters":{"recipe":"source_image_v1","commit":"a".repeat(40),
                "target":format!("linux-{}",std::env::consts::ARCH),"strategy":"auto","trigger":"operator","origin":"other"}
        })).unwrap(),
        lease: Lease::new(Grant { job, attempt: 1, until }).unwrap(),
        registered_at: now, coordinated: true, phase: Phase::CheckingOut,
        workspace: identity.workspace, context: identity.context, build: pipeline.build(),
        result: None, failure: None, release_receipt: None,
    }
}
