use agent_threads::drive::Mode;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    config::home::Home,
    remote_code::{
        command::{Decision, Offer, Request, Rule},
        launch::{ProcessIdentity, Reservation, started_fixture},
    },
};

use super::*;

struct Running {
    _temp: tempfile::TempDir,
    home: Home,
    saved: Reservation,
    process: ProcessIdentity,
    offer: Offer,
}

impl Running {
    fn start() -> Self {
        let (temp, home, saved, process) = started_fixture();
        let mut offer = Offer {
            process: process.id().to_string(),
            revision: 0,
            actions: vec![Action::Prompt, Action::Answer, Action::Stop],
            stop_targets: vec![Target::Interrupt {
                process: process.id().to_string(),
                turn: "turn-4".to_owned(),
            }],
            requests: vec![Request::Approval {
                id: "request-1".to_owned(),
                subject: "Run checks".to_owned(),
                details: "Run project checks".to_owned(),
                decisions: vec![Decision::AllowSession],
                rules: vec![Rule {
                    selector: "rule-1".to_owned(),
                    label: "Checks".to_owned(),
                }],
            }],
        };
        Store::at(&home)
            .revise_control(saved.request(), &process, &mut offer)
            .unwrap();
        Self {
            _temp: temp,
            home,
            saved,
            process,
            offer,
        }
    }

    fn command(&self, id: u128, payload: &Value) -> Delivered {
        let reserved = self.saved.request();
        serde_json::from_value(json!({
            "id": Uuid::from_u128(id), "account": self.saved.parameters().account, "device": reserved.device,
            "launch": reserved.launch, "thread": reserved.runner, "payload": payload,
        })).unwrap()
    }

    fn live(&self, threads_on: bool) -> LiveControl<'_> {
        LiveControl {
            process: &self.process,
            offer: &self.offer,
            mode: Mode::Ask,
            ceiling: Mode::Ask,
            threads_on,
            folder_permitted: true,
            publishable: true,
        }
    }
}

fn recorded(_: &Payload) -> Handoff {
    Handoff::Settled(ReceiptOutcome::Recorded { cause: () })
}

fn untouched(_: &Payload) -> Handoff {
    panic!("unexpected second or ineligible handoff")
}

fn outcome(consumed: Consumption) -> ReceiptOutcome {
    let Consumption::Settled(receipt) = consumed else {
        panic!("a settled command is required")
    };
    receipt.outcome
}

#[test]
fn queued_approval_is_exact_and_survives_switch_off() {
    let running = Running::start();
    let store = Store::at(&running.home);
    let live = running.live(false);
    let payload = |selector: &str| {
        json!({
            "kind": "answer", "process": running.offer.process, "revision": running.offer.revision,
            "request": "request-1", "answer": { "kind": "approval", "decision": "allow_session", "selector": selector },
        })
    };
    let substituted = running.command(1, &payload("Checks"));
    assert_eq!(
        outcome(store.consume(&substituted, &live, untouched).unwrap()),
        ReceiptOutcome::Refused {
            cause: Cause::InvalidAnswer
        }
    );
    let approved = running.command(2, &payload("rule-1"));
    let accepted = store.consume(&approved, &live, recorded).unwrap();
    assert_eq!(
        outcome(accepted.clone()),
        ReceiptOutcome::Recorded { cause: () }
    );
    let replay = Store::at(&running.home)
        .consume(&approved, &live, untouched)
        .unwrap();
    assert_eq!(replay, accepted);
    let duplicate_request = running.command(4, &payload("rule-1"));
    assert_eq!(
        outcome(store.consume(&duplicate_request, &live, untouched).unwrap()),
        ReceiptOutcome::Refused {
            cause: Cause::RequestResolved
        }
    );
    let prompt = running.command(3, &json!({"kind":"prompt","prompt":"fixture prompt"}));
    assert_eq!(
        outcome(store.consume(&prompt, &live, untouched).unwrap()),
        ReceiptOutcome::Refused {
            cause: Cause::ThreadsOff
        }
    );
}

#[test]
fn uncertain_handoff_and_old_receipt_survive_restart_and_later_commands() {
    let running = Running::start();
    let store = Store::at(&running.home);
    let live = running.live(true);
    let payload = || json!({"kind":"prompt","prompt":"fixture prompt"});
    let first = running.command(1, &payload());
    let uncertain = running.command(2, &payload());
    let accepted = store.consume(&first, &live, recorded).unwrap();
    assert_eq!(
        store
            .consume(&uncertain, &live, |_| Handoff::Uncertain)
            .unwrap(),
        Consumption::Reconciling
    );
    for id in 3..=1002 {
        store
            .consume(&running.command(id, &payload()), &live, recorded)
            .unwrap();
    }
    let reopened = Store::at(&running.home);
    assert_eq!(
        reopened.consume(&first, &live, untouched).unwrap(),
        accepted
    );
    assert_eq!(
        reopened.consume(&uncertain, &live, untouched).unwrap(),
        Consumption::Reconciling
    );
}

#[test]
fn stop_cannot_affect_a_successor_and_cancels_queued_prompts_individually() {
    let running = Running::start();
    let store = Store::at(&running.home);
    let live = running.live(true);
    let stop = |turn| {
        json!({
            "kind": "stop", "target": { "kind": "interrupt", "process": running.offer.process, "turn": turn },
        })
    };
    let stale = running.command(1, &stop("turn-3"));
    assert_eq!(
        outcome(store.consume(&stale, &live, untouched).unwrap()),
        ReceiptOutcome::Refused {
            cause: Cause::TargetStale
        }
    );
    let valid = running.command(2, &stop("turn-4"));
    assert_eq!(
        store
            .consume(&valid, &live, |_| Handoff::Uncertain)
            .unwrap(),
        Consumption::Reconciling
    );
    let prompt = running.command(3, &json!({"kind":"prompt","prompt":"fixture prompt"}));
    assert_eq!(
        outcome(store.cancel_prompt(&valid, &prompt).unwrap()),
        ReceiptOutcome::Refused {
            cause: Cause::CancelledByStop
        }
    );
    let later = running.command(
        4,
        &json!({"kind":"prompt","prompt":"another fixture prompt"}),
    );
    assert_eq!(
        store.consume(&later, &live, untouched).unwrap(),
        Consumption::Reconciling
    );
}

#[test]
fn a_mismatched_operator_gets_no_receipt_for_a_command_it_did_not_own() {
    let running = Running::start();
    let store = Store::at(&running.home);
    let live = running.live(true);
    let prompt = || json!({"kind":"prompt","prompt":"fixture prompt"});
    let authorized = running.command(1, &prompt());
    store.consume(&authorized, &live, recorded).unwrap();
    let mut wrong_owner = running.command(1, &prompt());
    wrong_owner.owner.account = Uuid::now_v7();
    assert!(matches!(
        store.consume(&wrong_owner, &live, untouched),
        Err(StoreError::Conflict)
    ));
    wrong_owner.id = Uuid::from_u128(2).to_string().try_into().unwrap();
    assert_eq!(
        outcome(store.consume(&wrong_owner, &live, untouched).unwrap()),
        ReceiptOutcome::Refused {
            cause: Cause::ThreadUnknown
        }
    );
}

#[test]
fn opaque_command_ids_replay_and_reject_substitution() {
    let running = Running::start();
    let store = Store::at(&running.home);
    let live = running.live(true);
    for id in ["../狀態/command".to_owned(), "é".repeat(128)] {
        let mut command = running.command(1, &json!({"kind":"prompt","prompt":"fixture prompt"}));
        command.id = id.clone().try_into().unwrap();
        let first = store.consume(&command, &live, recorded).unwrap();
        assert_eq!(
            Store::at(&running.home)
                .consume(&command, &live, untouched)
                .unwrap(),
            first
        );
        assert_eq!(serde_json::to_value(&command.id).unwrap(), id);
        command.payload = Payload::Prompt {
            prompt: "changed".to_owned(),
        };
        assert!(matches!(
            Store::at(&running.home).consume(&command, &live, untouched),
            Err(StoreError::Conflict)
        ));
    }
    for invalid in [
        String::new(),
        "é".repeat(129),
        "line
feed"
            .to_owned(),
    ] {
        assert!(super::super::CommandId::try_from(invalid).is_err());
    }
}
