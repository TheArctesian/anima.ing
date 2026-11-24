use super::{counter, row, writes};
use crate::threads::runner::thread::{Changes, Step, Thread};
use agent_threads::control::{Change, RunState};
use agent_threads::drive::{DriveEvent, Effort, Model, Program};
use serde_json::{Value, json};

fn reply(thread: &mut Thread, id: &str, result: &Value) -> Vec<Step> {
    thread.line(
        &json!({"jsonrpc":"2.0","id":id,"result":result}).to_string(),
        &mut counter(),
    )
}

fn ready() -> Thread {
    let mut thread = Thread::new(row(Program::Grok), None, None);
    thread.begin("hello");
    reply(
        &mut thread,
        "grok-initialize",
        &json!({"protocolVersion":1}),
    );
    reply(
        &mut thread,
        "grok-session",
        &json!({"sessionId":"session-1"}),
    );
    thread
}

#[test]
fn th119_no_prompt_precedes_acceptance_of_the_requested_model_and_effort() {
    let mut row = row(Program::Grok);
    row.model = Some("grok-example".into());
    row.effort = Some(Effort::High);
    let mut thread = Thread::new(row, None, None);
    assert_eq!(writes(&thread.begin("hello"))[0]["method"], "initialize");
    let steps = reply(
        &mut thread,
        "grok-initialize",
        &json!({"protocolVersion":1}),
    );
    assert_eq!(writes(&steps)[0]["method"], "session/new");
    let steps = reply(
        &mut thread,
        "grok-session",
        &json!({"sessionId":"session-1"}),
    );
    assert_eq!(writes(&steps)[0]["params"]["configId"], "model");
    let steps = reply(&mut thread, "grok-model", &json!({}));
    assert_eq!(writes(&steps)[0]["params"]["configId"], "reasoning_effort");
    let steps = reply(&mut thread, "grok-effort", &json!({}));
    assert_eq!(writes(&steps)[0]["method"], "session/prompt");
    assert_eq!(thread.row.state, RunState::Working);
}

#[test]
fn th119_a_completed_reply_stays_visible_and_the_next_queued_turn_starts() {
    let mut thread = ready();
    thread
        .queue(uuid::Uuid::from_u128(7), "next".into())
        .expect("queue");
    thread.line(&json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session-1","update":{
        "sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Hello"}
    }}}).to_string(),&mut counter());
    let steps = reply(
        &mut thread,
        "grok-turn-1",
        &json!({"stopReason":"end_turn"}),
    );
    assert!(steps.iter().any(|step| matches!(step,Step::Change(Change::Said { event:DriveEvent::Message {text,..} }) if text == "Hello")));
    assert_eq!(writes(&steps)[0]["params"]["prompt"][0]["text"], "next");
    assert_eq!(thread.row.state, RunState::Working);
    let key = thread.record_key();
    let mut restored = Thread::restore(thread.saved(), None);
    assert_eq!(restored.row.session.as_deref(), Some("session-1"));
    reply(
        &mut restored,
        "grok-turn-2",
        &json!({"stopReason":"end_turn"}),
    );
    assert_eq!(restored.row.state, RunState::Idle);
    assert_ne!(restored.record_key(), key);
}

#[test]
fn th119_failed_settings_send_no_prompt_and_idle_changes_need_acknowledgement() {
    let mut thread = ready();
    reply(
        &mut thread,
        "grok-turn-1",
        &json!({"stopReason":"end_turn"}),
    );
    let steps = thread
        .set(Changes {
            model: Some(Model::new("grok-next").expect("model")),
            ..Changes::default()
        })
        .expect("set");
    assert_eq!(thread.row.model, None);
    let id = writes(&steps)[0]["id"].as_str().expect("id").to_owned();
    reply(&mut thread, &id, &json!({}));
    assert_eq!(thread.row.model.as_deref(), Some("grok-next"));
    let mut other = Thread::new(row(Program::Grok), None, None);
    other.begin("must not go");
    let steps = other.line(
        r#"{"jsonrpc":"2.0","id":"grok-initialize","error":{"code":-1}}"#,
        &mut counter(),
    );
    assert!(steps.contains(&Step::EndProgram));
    assert_eq!(writes(&steps), Vec::<Value>::new());
}

#[test]
fn th119_startup_interrupt_and_out_of_order_replies_cannot_release_a_prompt() {
    let mut thread = Thread::new(row(Program::Grok), None, None);
    thread.begin("hello");
    let restored = Thread::restore(thread.saved(), None);
    assert!(restored.recovered().contains(&Step::EndProgram));
    assert!(thread.interrupt().contains(&Step::EndProgram));
    let mut other = Thread::new(row(Program::Grok), None, None);
    other.begin("hello");
    let steps = reply(&mut other, "grok-effort", &json!({}));
    assert!(steps.contains(&Step::EndProgram));
    assert_eq!(writes(&steps), Vec::<Value>::new());
}

#[test]
fn th119_permissions_of_a_different_session_are_never_presented() {
    let mut thread = ready();
    let steps = thread.line(&json!({"jsonrpc":"2.0","id":1,"method":"session/request_permission","params":{
        "sessionId":"different","toolCall":{"title":"write"},"options":[{"kind":"allow_once","optionId":"yes"}]
    }}).to_string(),&mut counter());
    assert!(steps.contains(&Step::EndProgram));
    assert_eq!(super::asked(&steps), Vec::<uuid::Uuid>::new());
}
