//! Run explicitly as root in a disposable container: the production socket is fixed.
use super::*;
use daemon_core::source_jobs::Grant;
use serde_json::json;
use std::{path::PathBuf, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixListener,
};

#[tokio::test]
#[ignore = "requires a disposable root Linux container with the production socket unused"]
async fn lost_lifecycle_replies_recover_from_durable_intent_over_the_real_socket() {
    assert_eq!(crate::config::private_file::running_user().unwrap(), 0);
    let temporary = tempfile::tempdir().unwrap();
    let home = Home::at(temporary.path().join("daemon"));
    let device = Uuid::now_v7();
    let origin = Origin::parse("https://example.invalid").unwrap();
    let job = tests::job();
    std::fs::create_dir_all(std::path::Path::new(SOCKET).parent().unwrap()).unwrap();
    // Bind refuses an occupied socket; this test never unlinks another owner's socket.
    let listener = UnixListener::bind(SOCKET).unwrap();
    let server = tokio::spawn(serve(
        listener,
        home.dir().join("source-work/journal.json"),
        job.clone(),
    ));
    let mut owner = Coordinator::open(&home, device, &origin).unwrap();
    assert!(matches!(
        owner.register(&job).await,
        Err(Error::Unconfirmed)
    ));
    let execution = owner.executions().unwrap()[0];
    drop(owner);
    let mut owner = Coordinator::open(&home, device, &origin).unwrap();
    assert_eq!(
        owner.reconcile(execution).await.unwrap()["phase"],
        "registered"
    );
    assert!(matches!(
        owner.start(execution).await,
        Err(Error::Unconfirmed)
    ));
    drop(owner);
    let lease = Grant {
        job: job.id,
        attempt: job.attempt,
        until: job.lease.until + chrono::TimeDelta::seconds(30),
    };
    recover_started_then_cancel(&home, (device, &origin), execution, lease).await;
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
    std::fs::remove_file(SOCKET).unwrap();
}

async fn recover_started_then_cancel(
    home: &Home,
    enrollment: (Uuid, &Origin),
    execution: Uuid,
    lease: Grant,
) {
    let (device, origin) = enrollment;
    let mut owner = Coordinator::open(home, device, origin).unwrap();
    assert_eq!(
        owner.reconcile(execution).await.unwrap()["phase"],
        "preparing"
    );
    owner.renew(execution, lease).await.unwrap();
    assert!(matches!(
        owner.cancel(execution).await,
        Err(Error::Unconfirmed)
    ));
    drop(owner);
    recover_cancelled(home, enrollment, execution, lease).await;
}

async fn recover_cancelled(
    home: &Home,
    enrollment: (Uuid, &Origin),
    execution: Uuid,
    lease: Grant,
) {
    let (device, origin) = enrollment;
    let mut owner = Coordinator::open(home, device, origin).unwrap();
    assert!(matches!(owner.start(execution).await, Err(Error::Conflict)));
    assert!(matches!(
        owner.renew(execution, lease).await,
        Err(Error::Conflict)
    ));
    assert_eq!(
        owner.reconcile(execution).await.unwrap()["phase"],
        "cancelled"
    );
    assert!(matches!(
        owner.status(execution).await,
        Err(Error::Unconfirmed)
    ));
    assert!(matches!(
        owner.status(Uuid::now_v7()).await,
        Err(Error::Conflict)
    ));
}

async fn serve(listener: UnixListener, path: PathBuf, job: WorkJob) {
    let mut identity = None;
    let cases = [
        ("register", "registered", true),
        ("status", "missing", true),
        ("register", "registered", false),
        ("start", "preparing", true),
        ("status", "registered", false),
        ("start", "preparing", false),
        ("renew", "preparing", false),
        ("cancel", "cancelled", true),
        ("status", "preparing", false),
        ("cancel", "cancelled", false),
        ("status", "changed-job", false),
    ];
    for (operation, phase, disconnect) in cases {
        let (mut stream, _) = tokio::time::timeout(Duration::from_secs(5), listener.accept())
            .await
            .unwrap()
            .unwrap();
        let mut frame = String::new();
        BufReader::new(&mut stream)
            .read_line(&mut frame)
            .await
            .unwrap();
        let request: Value = serde_json::from_str(&frame).unwrap();
        assert_eq!(request["action"]["operation"], operation);
        let stored: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let entry = &stored["entries"][0];
        assert_eq!(entry["device"], request["device"]);
        assert_eq!(entry["origin"], request["origin"]);
        assert_eq!(entry["execution"], request["action"]["execution"]);
        assert_eq!(entry["job"], serde_json::to_value(&job).unwrap());
        let execution = entry["execution"].clone();
        assert_eq!(identity.get_or_insert(execution.clone()), &execution);
        assert_intent(operation, entry, &request);
        if disconnect {
            continue;
        }
        let mut returned_job = job.clone();
        if phase == "changed-job" {
            returned_job.parameters["commit"] = json!("b".repeat(40));
        }
        let mut bytes = serde_json::to_vec(&json!({"ok":true,"value":{
            "execution":execution,"device":request["device"],"origin":request["origin"],
            "coordinated":true,"job":returned_job,"phase":phase
        }}))
        .unwrap();
        bytes.push(b'\n');
        stream.write_all(&bytes).await.unwrap();
    }
}

fn assert_intent(operation: &str, entry: &Value, request: &Value) {
    match operation {
        "register" => assert_eq!(request["action"]["item"], entry["job"]),
        "start" => assert_eq!(entry["start_requested"], true),
        "renew" => assert_eq!(entry["renewal"], request["action"]["lease"]),
        "cancel" => assert_eq!(entry["stop_requested"], true),
        _ => (),
    }
}

#[tokio::test]
#[ignore = "requires a disposable root Linux container with the production socket unused"]
async fn retirement_waits_for_expiry_and_the_exact_owner_acknowledgement() {
    let temporary = tempfile::tempdir().unwrap();
    let home = Home::at(temporary.path().join("daemon"));
    let device = Uuid::now_v7();
    let origin = Origin::parse("https://example.invalid").unwrap();
    let job = tests::job();
    let mut owner = Coordinator::open(&home, device, &origin).unwrap();
    let mut entry = owner
        .journal
        .reserve(device, &origin.as_text(), &job)
        .unwrap();
    entry.settled = true;
    owner.journal.replace(entry.clone()).unwrap();

    let mut cursor = 0;
    owner
        .prune(&mut cursor, job.lease.until - chrono::TimeDelta::seconds(1))
        .await
        .unwrap();
    assert!(owner.owned(entry.execution).is_ok());
    std::fs::create_dir_all(std::path::Path::new(SOCKET).parent().unwrap()).unwrap();
    let listener = UnixListener::bind(SOCKET).unwrap();
    let identity = json!({"execution":entry.execution,"job":job.id,"attempt":job.attempt});
    let server = tokio::spawn(async move {
        for matches in [false, true] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut frame = String::new();
            BufReader::new(&mut stream)
                .read_line(&mut frame)
                .await
                .unwrap();
            let request: Value = serde_json::from_str(&frame).unwrap();
            assert_eq!(
                request["action"],
                json!({"operation":"retire","identity":identity})
            );
            let mut response = identity.clone();
            if !matches {
                response["attempt"] = json!(2);
            }
            let reply = json!({"ok":true,"value":response});
            stream
                .write_all(format!("{reply}\n").as_bytes())
                .await
                .unwrap();
        }
    });

    owner.prune(&mut cursor, job.lease.until).await.unwrap();
    assert!(owner.owned(entry.execution).is_ok());
    owner.prune(&mut cursor, job.lease.until).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
    std::fs::remove_file(SOCKET).unwrap();
    drop(owner);

    let restored = Coordinator::open(&home, device, &origin).unwrap();
    assert!(matches!(
        restored.owned(entry.execution),
        Err(Error::Conflict)
    ));
}
