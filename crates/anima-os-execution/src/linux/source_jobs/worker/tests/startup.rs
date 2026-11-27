use super::*;
use crate::linux::workspace::{self, record};
use anima_os_maintenance::reservations::{Reservation, RuntimeOwner};

const STARTS: &str = "/run/workspace-starts";

#[tokio::test]
#[ignore = "requires disposable Btrfs and a Docker transport that raises pressure after create"]
async fn a_created_checkout_container_waits_and_starts_once() {
    let (record, pipeline) = startup_fixture(TimeDelta::milliseconds(900)).await;
    workspace::prepare_source(pipeline, 1 << 30).await.unwrap();
    let reservation = reservation(&record);
    let grant = record.lease.grant;
    let mut start = tokio::spawn(workspace::start_source(
        pipeline,
        reservation.clone(),
        grant,
    ));
    wait_for_hold(&mut start).await;
    let current = record::required(record.workspace).unwrap();
    assert!(current.phase == record::Phase::Starting);
    assert_eq!(current.generation, 1);
    assert_eq!(current.run, Some(record.execution));
    assert!(
        Path::new("/persist/anima-admission/reservations")
            .join(format!("{}.json", record.execution))
            .is_file()
    );
    assert!(!Path::new(STARTS).exists());
    drop(Admission::cleanup().expect("the staged workspace must leave admission free"));
    renew(&record).await;
    clear_hold();
    let result = finish(start).await.unwrap();
    assert_eq!(result["workspace"]["phase"], "running");
    assert_eq!(result["runtime"]["running"], true);
    workspace::start_source(pipeline, reservation, grant)
        .await
        .unwrap();
    assert_eq!(fs::read_to_string(STARTS).unwrap(), format!("{:064}\n", 2));
    assert_eq!(
        fs::read_to_string("/run/workspace-creates")
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[tokio::test]
#[ignore = "requires disposable Btrfs and a Docker transport that raises pressure after create"]
async fn cancelling_a_held_checkout_prevents_start() {
    let (record, pipeline) = startup_fixture(TimeDelta::seconds(30)).await;
    workspace::prepare_source(pipeline, 1 << 30).await.unwrap();
    let mut start = tokio::spawn(workspace::start_source(
        pipeline,
        reservation(&record),
        record.lease.grant,
    ));
    wait_for_hold(&mut start).await;
    mark_stopping(record.execution).unwrap();
    clear_hold();
    let error = finish(start).await.unwrap_err();
    assert!(
        error
            .to_string()
            .contains("no longer owns this checkout workspace")
    );
    assert!(!Path::new(STARTS).exists());
}

#[tokio::test]
#[ignore = "requires disposable Btrfs and the workspace Docker transport fixture"]
async fn checkout_waits_for_a_pressure_sample_before_creating_a_home() {
    let (record, pipeline) = startup_fixture(TimeDelta::seconds(30)).await;
    fs::write("/run/workspace-no-late-hold", b"").unwrap();
    fs::remove_file(Path::new(fixture::PRESSURE).join("clear")).unwrap();
    let resources = reservation(&record).resources;
    let mut start = tokio::spawn(super::super::checkout_workspace::start(
        pipeline,
        record.lease.grant,
        resources,
        1 << 30,
    ));
    let early = tokio::time::timeout(Duration::from_millis(150), &mut start).await;
    assert!(
        early.is_err(),
        "checkout ended without a current sample: {early:?}"
    );
    assert!(record::read(record.workspace).unwrap().is_none());
    assert!(!record::home(record.workspace).exists());
    drop(Admission::cleanup().expect("the waiting checkout must leave admission free"));
    fixture::clear_sample();
    finish(start).await.unwrap();
    assert!(record::required(record.workspace).unwrap().phase == record::Phase::Running);
    assert_eq!(fs::read_to_string(STARTS).unwrap().lines().count(), 1);
}

async fn renew(record: &Record) {
    let lease = Grant {
        until: Utc::now() + TimeDelta::seconds(60),
        ..record.lease.grant
    };
    let renewed = Request::Renew {
        execution: record.execution,
        lease,
    }
    .execute()
    .await
    .unwrap();
    assert_eq!(renewed["phase"], "preparing");
}

async fn wait_for_hold(start: &mut JoinHandle<Result<serde_json::Value>>) {
    let sample = tokio::time::timeout(Duration::from_secs(5), async {
        while !hold().exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
    tokio::select! {
        result = &mut *start => panic!("checkout ended before it could wait: {result:?}"),
        result = sample => result.expect("the Docker fixture must hold pressure"),
    }
    let early = tokio::time::timeout(Duration::from_millis(150), start).await;
    assert!(
        early.is_err(),
        "checkout ended during the pressure hold: {early:?}"
    );
}

fn hold() -> std::path::PathBuf {
    Path::new(fixture::PRESSURE).join("active")
}

fn clear_hold() {
    fixture::clear_sample();
    fs::remove_file(hold()).unwrap();
}

fn reservation(record: &Record) -> Reservation {
    Reservation {
        schema: 1,
        runtime: RuntimeOwner::Docker,
        run: record.execution,
        generation: 1,
        resources: super::super::limits(record).unwrap().0,
    }
}

async fn startup_fixture(span: TimeDelta) -> (Record, Pipeline) {
    let identity = Identity {
        workspace: Uuid::now_v7(),
        ..fixture::prepare_workspace().await
    };
    fs::create_dir_all("/persist/anima-admission/reservations").unwrap();
    let pipeline = accepted_pipeline(identity.workspace);
    work_policy::Request::WorkPolicySet {
        revision: 0,
        enabled: true,
        repositories: "example/source".into(),
    }
    .execute()
    .unwrap();
    let mut record = checked_out_job(pipeline, identity, span);
    record.phase = Phase::Preparing;
    save_fixture(&record);
    (record, pipeline)
}
