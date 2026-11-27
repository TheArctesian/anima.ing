use super::*;
use crate::linux::source_jobs::{cleanup, stopping, worker};

#[tokio::test]
#[ignore = "requires disposable Btrfs and a systemd observation fixture for an OOM-killed build"]
async fn an_oom_killed_build_keeps_its_cause_after_release() {
    let record = exhausted_build().await;
    let failure = worker::wait(record.execution, record.build)
        .await
        .unwrap_err();
    worker::mark_failed(record.execution, &failure).unwrap();
    stopping::finish(record.execution).await.unwrap();
    let released = cleanup::release(record.execution, true).await.unwrap();
    assert_eq!(released["phase"], "released");
    assert_eq!(released["failure"], "limit_memory");
    assert!(released["result"].is_null());
    let reservation = format!(
        "/persist/anima-admission/reservations/{}.json",
        record.build
    );
    assert!(!Path::new(&reservation).exists());
}

async fn exhausted_build() -> Record {
    let identity = fixture::prepare_workspace().await;
    let pipeline = accepted_pipeline(identity.workspace);
    let mut record = checked_out_job(pipeline, identity, TimeDelta::seconds(30));
    record.phase = Phase::Building;
    save_fixture(&record);
    fs::create_dir_all(Path::new(builds::store::ROOT).join("locks")).unwrap();
    fs::create_dir(builds::store::base(record.build)).unwrap();
    fs::create_dir_all("/persist/anima-admission/reservations").unwrap();

    let resources = worker::limits(&record).unwrap().0;
    let attempt = serde_json::from_value(json!({
        "schema":1, "id":record.build, "supervisor":record.execution,
        "workspace":record.workspace, "generation":1, "context":record.context, "resources":resources,
        "strategy":"railpack", "phase":"running", "result":null
    }))
    .unwrap();
    builds::store::save(&attempt).unwrap();
    let reservation = json!({"schema":1,"run":record.build,"generation":1,
        "runtime":"systemd-build","resources":resources});
    let path = format!(
        "/persist/anima-admission/reservations/{}.json",
        record.build
    );
    fs::write(path, serde_json::to_vec(&reservation).unwrap()).unwrap();
    record
}
