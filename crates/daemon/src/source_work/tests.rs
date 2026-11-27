use super::*;
use daemon_core::wire::WorkState;
use serde_json::json;

pub(super) fn job() -> WorkJob {
    serde_json::from_value(json!({
        "id":Uuid::now_v7(),"type":"build","kind":"container","attempt":1,
        "lease":{"until":"2026-11-24T18:00:00Z"},
        "limits":{"seconds":300,"idleSeconds":60,"memoryBytes":536_870_912,
            "diskBytes":22_548_578_304_u64,"orphanSeconds":60},
        "project":null,"repository":{"forge":"github","owner":"example","name":"sample"},
        "secrets":[],"parameters":{"recipe":"source_image_v1","commit":"a".repeat(40),
            "target":"linux-aarch64","strategy":"auto","trigger":"operator","origin":"default_branch"}
    })).unwrap()
}

#[test]
fn durable_dispatch_reuses_identity_after_restart_and_rejects_conflicting_retries() {
    let temporary = tempfile::tempdir().unwrap();
    let home = Home::at(temporary.path().join("daemon"));
    let device = Uuid::now_v7();
    let origin = Origin::parse("https://example.invalid").unwrap();
    let job = job();
    let mut owner = Coordinator::open(&home, device, &origin).unwrap();
    let entry = owner
        .journal
        .reserve(device, &origin.as_text(), &job)
        .unwrap();
    assert!(matches!(
        Coordinator::open(&home, device, &origin),
        Err(Error::Busy)
    ));
    drop(owner);
    let mut owner = Coordinator::open(&home, device, &origin).unwrap();
    let mut retry = job.clone();
    retry.lease.until += chrono::TimeDelta::seconds(30);
    let restored = owner
        .journal
        .reserve(device, &origin.as_text(), &retry)
        .unwrap();
    assert_eq!(restored.execution, entry.execution);
    assert_eq!(restored.job.lease, job.lease);
    retry.parameters["commit"] = json!("b".repeat(40));
    assert!(matches!(
        owner.journal.reserve(device, &origin.as_text(), &retry),
        Err(Error::Conflict)
    ));
    assert_eq!(
        owner.holds().unwrap(),
        vec![Held {
            id: job.id,
            attempt: 1
        }]
    );
    drop(owner);
    let other = Coordinator::open(&home, Uuid::now_v7(), &origin).unwrap();
    assert_eq!(other.holds().unwrap(), []);
    assert!(matches!(other.owned(entry.execution), Err(Error::Conflict)));
}

#[test]
fn report_outbox_replays_after_restart_and_requires_the_exact_acknowledgement() {
    let temporary = tempfile::tempdir().unwrap();
    let home = Home::at(temporary.path().join("daemon"));
    let device = Uuid::now_v7();
    let origin = Origin::parse("https://example.invalid").unwrap();
    let job = job();
    let mut owner = Coordinator::open(&home, device, &origin).unwrap();
    let execution = owner
        .journal
        .reserve(device, &origin.as_text(), &job)
        .unwrap()
        .execution;
    let running = WorkReport {
        id: job.id,
        attempt: 1,
        state: WorkState::Running,
        log: None,
        result: None,
    };
    owner.report(execution, running.clone()).unwrap();
    drop(owner);
    let mut owner = Coordinator::open(&home, device, &origin).unwrap();
    assert_eq!(owner.pending().unwrap(), vec![(execution, running.clone())]);
    let done = WorkReport {
        state: WorkState::Done,
        result: Some(daemon_core::wire::WorkResult {
            outcome: "succeeded".into(),
            cause: None,
            seconds: 10,
            health: None,
            cleanup: daemon_core::wire::Cleanup {
                isolation: "deleted".into(),
                checkout: "deleted".into(),
                secrets: "none".into(),
                keychain: "none".into(),
            },
        }),
        ..running.clone()
    };
    owner.report(execution, done.clone()).unwrap();
    let response = daemon_core::wire::Reported {
        lease: job.lease,
        stop: None,
        log_end: None,
    };
    assert!(owner.acknowledge(execution, &running, response).is_err());
    assert!(owner.report(execution, running).is_err());
    assert_eq!(owner.holds().unwrap().len(), 1);
    owner.acknowledge(execution, &done, response).unwrap();
    drop(owner);
    let mut owner = Coordinator::open(&home, device, &origin).unwrap();
    assert_eq!(owner.holds().unwrap(), []);
    assert_eq!(owner.pending().unwrap(), []);
    assert!(
        owner
            .journal
            .reserve(device, &origin.as_text(), &job)
            .is_err()
    );
}

#[test]
fn invalid_or_linked_journal_never_becomes_an_empty_claim_inventory() {
    let temporary = tempfile::tempdir().unwrap();
    let home = Home::at(temporary.path().join("daemon"));
    let device = Uuid::now_v7();
    let origin = Origin::parse("https://example.invalid").unwrap();
    drop(Coordinator::open(&home, device, &origin).unwrap());
    let dir = home.dir().join("source-work");
    crate::config::private_file::write(&dir, "journal.json", b"{bad}").unwrap();
    assert!(Coordinator::open(&home, device, &origin).is_err());
    std::fs::remove_file(dir.join("journal.json")).unwrap();
    crate::config::private_file::write(&dir, "elsewhere", br#"{"schema":1,"entries":[]}"#).unwrap();
    std::os::unix::fs::symlink(dir.join("elsewhere"), dir.join("journal.json")).unwrap();
    assert!(Coordinator::open(&home, device, &origin).is_err());
}
