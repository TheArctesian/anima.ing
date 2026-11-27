use super::{Entry, History, Outcome};
use crate::linux::operator_activity::{Action, Actor};
use nix::unistd::Uid;
use std::{fs, os::unix::fs::PermissionsExt, thread};
use uuid::Uuid;

fn folder() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    directory
}

fn record(action: Action) -> Entry {
    Entry {
        id: Uuid::now_v7(),
        actor: Actor {
            uid: Some(1000),
            account: Some("operator".to_owned()),
        },
        action,
        at_ms: 1_795_518_000_000,
        outcome: Outcome::Requested,
    }
}

#[test]
fn reopening_preserves_an_unconfirmed_request_with_bounded_history() {
    let directory = folder();
    let owner = Uid::effective().as_raw();
    let original = History::new(directory.path(), owner);
    let oldest = record(Action::Connect);
    let evicted = oldest.id;
    original.append(oldest).unwrap();
    for _ in 0..49 {
        original.append(record(Action::HoldWork)).unwrap();
    }
    let latest = record(Action::Restart);
    let id = latest.id;
    original.append(latest).unwrap();

    let reopened = History::new(directory.path(), owner);
    let rows = reopened.read().unwrap();
    assert_eq!(rows.len(), 50);
    assert!(!rows.iter().any(|row| row.id == evicted));
    let first = &rows[0];
    assert_eq!(first.id, id);
    assert_eq!(first.actor.label(), "operator");
    assert!(matches!(first.outcome, Outcome::Requested));
    reopened.finish(id, Outcome::Accepted).unwrap();
    assert!(matches!(
        original.read().unwrap()[0].outcome,
        Outcome::Accepted
    ));
}

#[test]
fn concurrent_sessions_keep_both_receipts_and_readers_refuse_unsafe_files() {
    let directory = folder();
    let history = History::new(directory.path(), Uid::effective().as_raw());
    let reboot = record(Action::Restart);
    let reboot_id = reboot.id;
    let drain = record(Action::HoldWork);
    let drain_id = drain.id;
    thread::scope(|scope| {
        scope.spawn(|| history.append(reboot).unwrap());
        scope.spawn(|| {
            history.append(drain).unwrap();
            history.finish(drain_id, Outcome::Accepted).unwrap();
        });
    });
    let rows = history.read().unwrap();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .any(|row| row.id == drain_id && matches!(row.outcome, Outcome::Accepted))
    );
    assert!(
        rows.iter()
            .any(|row| row.id == reboot_id && matches!(row.outcome, Outcome::Requested))
    );

    let path = directory.path().join("actions.json");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(history.read().is_err());
}
