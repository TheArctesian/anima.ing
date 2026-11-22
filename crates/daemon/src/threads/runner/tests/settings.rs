//! A change of the model, the effort or the mode after the start (TH110): from the next turn.

use agent_threads::control::{Change, Refusal};
use agent_threads::drive::{DriveEvent, Effort, Mode, Model};

use super::{claude, codex_in_a_turn, counter, held_claude, writes};
use crate::threads::runner::thread::{Changes, Step, Thread};

const DONE: &str = r#"{"type":"result","subtype":"success","is_error":false,"result":"ok","terminal_reason":"completed"}"#;
const IDLE: &str = r#"{"type":"system","subtype":"session_state_changed","state":"idle"}"#;

fn model(name: &str) -> Model {
    Model::new(name).expect("a model")
}

fn subtypes(steps: &[Step]) -> Vec<String> {
    writes(steps)
        .iter()
        .filter_map(|line| line["request"]["subtype"].as_str().map(str::to_owned))
        .collect()
}

fn idle_claude() -> Thread {
    let mut thread = held_claude();
    thread.line(DONE, &mut counter());
    thread.line(IDLE, &mut counter());
    thread
}

#[test]
fn th110_claude_code_takes_the_model_and_the_mode_at_once_between_turns() {
    let mut thread = idle_claude();
    let steps = thread
        .set(Changes {
            model: Some(model("sonnet")),
            mode: Some(Mode::AcceptEdits),
            ..Changes::default()
        })
        .expect("changed");
    assert_eq!(subtypes(&steps), ["set_model", "set_permission_mode"]);
    assert_eq!(writes(&steps)[1]["request"]["mode"], "acceptEdits");
    assert_eq!(thread.row.model.as_deref(), Some("sonnet"));
    assert_eq!(thread.row.mode, Mode::AcceptEdits);
}

#[test]
fn th110_a_change_during_a_turn_goes_when_the_turn_ends_before_a_queued_prompt() {
    let mut thread = held_claude();
    let steps = thread
        .set(Changes {
            mode: Some(Mode::Ask),
            ..Changes::default()
        })
        .expect("changed");
    assert_eq!(subtypes(&steps), Vec::<String>::new(), "a turn runs");
    thread
        .queue(uuid::Uuid::from_u128(1), "next".to_owned())
        .expect("queued");
    let ended = writes(&thread.line(DONE, &mut counter()));
    assert_eq!(ended[0]["request"]["subtype"], "set_permission_mode");
    assert_eq!(
        ended[1]["message"]["content"], "next",
        "the queued prompt goes after it"
    );
}

#[test]
fn th110_claude_code_refuses_a_change_of_the_effort_with_words_and_nothing_changes() {
    let mut thread = idle_claude();
    let refused = thread.set(Changes {
        model: Some(model("opus")),
        effort: Some(Effort::High),
        ..Changes::default()
    });
    assert!(matches!(refused, Err(Refusal::CannotChange { said }) if said.contains("effort")));
    assert_eq!(
        thread.row.model, None,
        "a call with a refused part changes nothing"
    );
    assert_eq!(thread.row.effort, None);
}

#[test]
fn th110_a_refusal_of_claude_code_is_said_to_the_person() {
    let mut thread = idle_claude();
    let steps = thread
        .set(Changes {
            model: Some(model("nope")),
            ..Changes::default()
        })
        .expect("changed");
    let id = writes(&steps)[0]["request_id"].clone();
    let answer = format!(
        r#"{{"type":"control_response","response":{{"subtype":"error","request_id":{id},"error":"no such model"}}}}"#
    );
    let said = thread.line(&answer, &mut counter());
    assert!(said.iter().any(|step| matches!(
        step,
        Step::Change(Change::Said { event: DriveEvent::Notice { text } }) if text.contains("model")
    )));
}

#[test]
fn th110_codex_carries_the_change_on_each_later_turn_and_refuses_plan_mode() {
    let mut ids = counter();
    let (mut thread, _) = codex_in_a_turn(&mut ids);
    assert!(matches!(
        thread.set(Changes {
            mode: Some(Mode::Plan),
            ..Changes::default()
        }),
        Err(Refusal::CannotChange { .. })
    ));
    let steps = thread
        .set(Changes {
            model: Some(model("gpt-5.6-sol")),
            effort: Some(Effort::Medium),
            mode: Some(Mode::Ask),
        })
        .expect("changed");
    assert_eq!(writes(&steps).len(), 0, "nothing goes to the running turn");
    assert_eq!(thread.row.effort, Some(Effort::Medium));
    thread.line(
        r#"{"method":"turn/completed","params":{"turn":{"id":"u1","status":"completed"}}}"#,
        &mut ids,
    );
    for _ in 0..2 {
        let next = thread.send("go on", true).expect("running");
        let params = &serde_json::from_str::<serde_json::Value>(&next[0]).expect("JSON")["params"];
        assert_eq!(params["model"], "gpt-5.6-sol");
        assert_eq!(params["effort"], "medium");
        assert_eq!(params["approvalPolicy"], "untrusted");
        thread.line(
            r#"{"method":"turn/completed","params":{"turn":{"id":"u1","status":"completed"}}}"#,
            &mut ids,
        );
    }
}

#[test]
fn th110_a_thread_that_does_not_run_changes_nothing() {
    let mut thread = claude();
    assert_eq!(
        thread.set(Changes {
            mode: Some(Mode::Ask),
            ..Changes::default()
        }),
        Err(Refusal::NotRunning)
    );
}
