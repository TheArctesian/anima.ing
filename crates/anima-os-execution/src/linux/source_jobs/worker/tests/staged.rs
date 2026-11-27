use super::*;

#[tokio::test]
#[ignore = "requires disposable Btrfs and a build transport that holds pressure during preparation"]
async fn a_prepared_build_waits_for_clear_pressure_and_dispatches_once() {
    let (record, pipeline, mut worker) = waiting_build().await;
    let base = builds::store::base(record.build);
    assert!(base.join("work").is_dir());
    assert!(base.join("request.json").is_file());
    let receipts = Path::new("/persist/anima-admission/reservations");
    assert!(receipts.join(format!("{}.json", record.build)).is_file());
    drop(Admission::cleanup().expect("the held build must leave admission free"));
    assert!(!Path::new("/run/build-dispatches").exists());

    let lease = Grant {
        until: Utc::now() + TimeDelta::seconds(60),
        ..record.lease.grant
    };
    Request::Renew {
        execution: record.execution,
        lease,
    }
    .execute()
    .await
    .unwrap();
    assert_eq!(
        builds::store::required(record.build)
            .unwrap()
            .lease
            .unwrap()
            .grant
            .until,
        lease.until
    );
    let admission = Admission::cleanup().unwrap();
    clear_hold();
    let busy = tokio::time::timeout(Duration::from_millis(1100), &mut worker).await;
    assert!(
        busy.is_err(),
        "the staged build ended while admission was busy: {busy:?}"
    );
    drop(admission);
    assert_eq!(finish(worker).await.unwrap(), record.build);
    assert_eq!(
        builds::store::required(record.build).unwrap().phase,
        "running"
    );
    let resources = super::super::limits(&record).unwrap().0;
    assert_eq!(
        start_build(record.execution, pipeline, resources)
            .await
            .unwrap(),
        record.build
    );
    assert_eq!(
        fs::read_to_string("/run/build-dispatches").unwrap(),
        format!("--unit=anima-build-{}.service\n", record.build)
    );
}

#[tokio::test]
#[ignore = "requires disposable Btrfs and a build transport that holds pressure during preparation"]
async fn a_cancelled_prepared_build_never_dispatches() {
    let (record, _, worker) = waiting_build().await;
    mark_stopping(record.execution).unwrap();
    clear_hold();
    let failure = finish(worker).await.unwrap_err();
    assert!(failure.to_string().contains("no longer owns this build"));
    assert!(!Path::new("/run/build-dispatches").exists());
    assert!(snapshot(record.execution).unwrap().phase == Phase::Cancelling);
}

fn clear_hold() {
    fixture::clear_sample();
    fs::remove_file(Path::new(fixture::PRESSURE).join("active")).unwrap();
}

async fn waiting_build() -> (Record, Pipeline, JoinHandle<Result<Uuid>>) {
    let (record, pipeline) = staged_fixture().await;
    let resources = super::super::limits(&record).unwrap().0;
    let mut worker = tokio::spawn(start_build(record.execution, pipeline, resources));
    let hold = Path::new(fixture::PRESSURE).join("active");
    let observing = tokio::time::timeout(Duration::from_secs(5), async {
        while !hold.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
    tokio::select! {
        result = &mut worker => panic!("build ended before the pressure hold: {result:?}"),
        result = observing => result.expect("the preparation transport must hold pressure"),
    }
    let early = tokio::time::timeout(Duration::from_millis(150), &mut worker).await;
    assert!(
        early.is_err(),
        "the staged build ended during a hold: {early:?}"
    );
    (record, pipeline, worker)
}

async fn staged_fixture() -> (Record, Pipeline) {
    let identity = fixture::prepare_workspace().await;
    let pipeline = accepted_pipeline(identity.workspace);
    fs::create_dir_all(Path::new(builds::store::ROOT).join("locks")).unwrap();
    fs::create_dir_all("/persist/anima-admission/reservations").unwrap();
    let captured = context::capture(identity, Some(pipeline)).await.unwrap();
    assert_eq!(captured["phase"], "captured");
    work_policy::Request::WorkPolicySet {
        revision: 0,
        enabled: true,
        repositories: "example/source".into(),
    }
    .execute()
    .unwrap();
    let record = checked_out_job(pipeline, identity, TimeDelta::seconds(20));
    save_fixture(&record);
    (record, pipeline)
}
