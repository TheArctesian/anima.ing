use std::{cell::Cell, time::Duration};

use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

use super::*;
use crate::remote_code::launch::{DispatchExchange, PreparingReport, ProcessIdentity};
use crate::{
    config::{home::Home, origin::Origin},
    remote_code::wire::{ClaimedLaunch, Granted},
};

fn ticks(micros: u64) -> Sample {
    Sample {
        ticks: Duration::from_micros(micros),
        resolution: Duration::from_micros(1),
    }
}

fn launch() -> (tempfile::TempDir, Home, ClaimedLaunch, Uuid) {
    let dir = tempfile::tempdir().unwrap();
    let home = Home::at(dir.path().join("daemon"));
    home.ensure().unwrap();
    let claim = serde_json::from_value(json!({
        "id": Uuid::now_v7(), "type": "agent_thread", "kind": "person", "attempt": 1,
        "lease": { "until": "2026-11-25T20:00:00Z" },
        "limits": { "seconds": 120, "idleSeconds": 0, "memoryBytes": 0,
            "diskBytes": 0, "orphanSeconds": 0 },
        "project": Uuid::now_v7(),
        "parameters": { "launch": Uuid::now_v7(), "account": Uuid::now_v7(), "program": "codex", "folder": dir.path(), "access": "ask" }
    })).unwrap();
    (dir, home, claim, Uuid::now_v7())
}

fn grant(reservation: &Reservation) -> DispatchGrant {
    let exchange = DispatchExchange {
        request: *reservation.request(),
        start: ticks(100_000),
    };
    exchange
        .accepted(Granted {
            authorization: Uuid::from_u128(97),
            valid_until: Utc::now(),
            remaining_ms: 10_000,
        })
        .unwrap()
}

fn prepared(saved: &Reservation) -> (Mode, Command) {
    let mut command = Command::new("/bin/sleep");
    command
        .arg("30")
        .current_dir(&saved.parameters().folder)
        .kill_on_drop(true);
    (Mode::Ask, command)
}

fn progress(dispatch: &Dispatch) -> Progress {
    match dispatch {
        Dispatch::NoChild(progress) => *progress,
        Dispatch::Spawned(_) => panic!("unexpected process start"),
    }
}

#[tokio::test]
async fn dispatch_charges_the_response_delay_and_rechecks_after_the_marker() {
    let (_dir, home, claim, owner) = launch();
    let store = Store::at(&home);
    let reserved = store
        .reserve((owner, &Origin::default_hub()), &claim)
        .unwrap();
    let retry = Store::at(&home)
        .reserve((owner, &Origin::default_hub()), &claim)
        .unwrap();
    assert!(reserved.request() == retry.request());
    let other = Origin::parse_kept("https://other.example").unwrap();
    assert!(matches!(
        store.reserve((owner, &other), &claim),
        Err(StoreError::Conflict)
    ));
    let lease = PreparingReport(ticks(80_000)).accepted(*reserved.request());
    let time = Cell::new(ticks(10_100_000));
    let delayed = store
        .spawn_with_clock(
            (grant(&reserved), &lease),
            (|_| Ok(Mode::Ask), || Some(time.get())),
            prepared(&reserved),
        )
        .unwrap();
    assert_eq!(progress(&delayed), Progress::WaitingForGrant);

    time.set(ticks(100_700));
    let marked = store
        .spawn_with_clock(
            (grant(&reserved), &lease),
            (|_| Ok(Mode::Ask), || Some(time.replace(ticks(10_100_000)))),
            prepared(&reserved),
        )
        .unwrap();
    assert_eq!(progress(&marked), Progress::Reconciling);
    let reopened = Store::at(&home);
    assert_eq!(
        reopened
            .close(reserved.request(), NotCreated::Expired)
            .unwrap(),
        Progress::Reconciling
    );
    assert!(
        reopened
            .reservation(reserved.request())
            .unwrap()
            .evidence()
            .is_none()
    );
    assert_eq!(
        progress(
            &reopened
                .spawn_with_clock(
                    (grant(&reserved), &lease),
                    (|_| Ok(Mode::Ask), || Some(ticks(100_800))),
                    prepared(&reserved)
                )
                .unwrap()
        ),
        Progress::Reconciling
    );
}

#[tokio::test]
async fn original_verified_child_recovers_without_a_second_os_call() {
    let (_dir, home, claim, device) = launch();
    let store = Store::at(&home);
    let reserved = store
        .reserve((device, &Origin::default_hub()), &claim)
        .unwrap();
    let lease = PreparingReport(ticks(80_000)).accepted(*reserved.request());
    let spawn = || {
        store
            .spawn_with_clock(
                (grant(&reserved), &lease),
                (|_| Ok(Mode::Ask), || Some(ticks(201_000))),
                prepared(&reserved),
            )
            .unwrap()
    };
    let Dispatch::Spawned(mut child) = spawn() else {
        panic!("fixture did not start")
    };
    let reopened = Store::at(&home);
    assert_eq!(progress(&spawn()), Progress::Reconciling);
    let unavailable = reopened.lock().unwrap();
    assert!(matches!(child.record(&reopened), Err(StoreError::Busy)));
    let Err(mut child) = (*child).into_child() else {
        panic!("storage failure unsafely detached the child")
    };
    drop(unavailable);
    assert_eq!(child.record(&reopened).unwrap(), Progress::Created);
    assert_eq!(child.record(&reopened).unwrap(), Progress::Created);
    let Ok((mut child, _)) = child.into_child() else {
        panic!("creation was not durable")
    };
    child.kill().await.unwrap();
    let evidence = reopened
        .reservation(reserved.request())
        .unwrap()
        .evidence()
        .unwrap();
    let body = serde_json::to_value(evidence).unwrap();
    assert_eq!(body["outcome"], "created");
    assert_eq!(body["thread"], reserved.request().runner.to_string());
    assert_eq!(body["attempt"], 1);
}

#[test]
fn cancellation_before_the_marker_has_durable_original_attempt_evidence() {
    let (_dir, home, claim, owner) = launch();
    let store = Store::at(&home);
    let reserved = store
        .reserve((owner, &Origin::default_hub()), &claim)
        .unwrap();
    assert_eq!(
        store
            .close(reserved.request(), NotCreated::Stopped)
            .unwrap(),
        Progress::Closed(NotCreated::Stopped)
    );
    let reopened = Store::at(&home);
    let evidence = reopened
        .reservation(reserved.request())
        .unwrap()
        .evidence()
        .unwrap();
    let body = serde_json::to_value(evidence).unwrap();
    assert_eq!(body["outcome"], "not_created");
    assert_eq!(body["cause"], "stopped");
    assert_eq!(body["work"], claim.id.to_string());
    assert!(body.get("thread").is_none());
}

pub(in crate::remote_code) fn started_fixture()
-> (tempfile::TempDir, Home, Reservation, ProcessIdentity) {
    let (dir, home, claim, device) = launch();
    let store = Store::at(&home);
    let reserved = store
        .reserve((device, &Origin::default_hub()), &claim)
        .unwrap();
    let process = ProcessIdentity(Uuid::now_v7());
    let mut locked = store.lock().unwrap();
    let saved = find(&mut locked.journal, reserved.request()).unwrap();
    saved.state = LaunchState::Created {
        grant: GrantReference {
            authorization: Uuid::from_u128(97),
            valid_until: Utc::now(),
        },
        access: Mode::Ask,
        process: process.clone(),
        withdrawn: false,
        ended: false,
    };
    locked.save().unwrap();
    (dir, home, reserved, process)
}

#[test]
fn narrowing_the_owner_ceiling_during_the_marker_write_prevents_spawn() {
    let (_dir, home, claim, device) = launch();
    let store = Store::at(&home);
    let reserved = store
        .reserve((device, &Origin::default_hub()), &claim)
        .unwrap();
    let lease = PreparingReport(ticks(80_000)).accepted(*reserved.request());
    let second_check = Cell::new(false);
    let check = |_: &Reservation| {
        let already_marked = second_check.replace(true);
        Ok(if already_marked {
            Mode::Plan
        } else {
            Mode::Ask
        })
    };
    let actual = store
        .spawn_with_clock(
            (grant(&reserved), &lease),
            (check, || Some(ticks(200_000))),
            prepared(&reserved),
        )
        .unwrap();
    assert_eq!(progress(&actual), Progress::Reconciling);
    assert!(
        store
            .reservation(reserved.request())
            .unwrap()
            .evidence()
            .is_none()
    );
}

#[cfg(target_os = "linux")]
mod cleanup;
