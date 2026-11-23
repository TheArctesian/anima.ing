//! The diff, the plan, the use of tokens and the models (TH101 to TH104), from recorded lines.

use super::*;

fn usage_of(events: &[DriveEvent]) -> Vec<&Usage> {
    events
        .iter()
        .filter_map(|event| match event {
            DriveEvent::Usage { usage } => Some(usage),
            _ => None,
        })
        .collect()
}

#[test]
fn th101_th103_a_recorded_codex_turn_gives_its_diff_and_its_use_of_tokens() {
    let events = codex(CODEX_FULL);
    let diffs: Vec<&String> = events
        .iter()
        .filter_map(|event| match event {
            DriveEvent::TurnDiff { diff, turn } => {
                assert!(turn.is_some(), "a diff names its turn");
                Some(diff)
            }
            _ => None,
        })
        .collect();
    assert_eq!(diffs.len(), 5, "each turn/diff/updated of the record");
    assert!(
        diffs
            .last()
            .expect("one")
            .contains("+++ b/note.txt\n@@ -0,0 +1 @@\n+ok")
    );
    let usage = usage_of(&events);
    assert_eq!(usage.len(), 5);
    let first = usage[0];
    assert_eq!(first.window, Some(475_000));
    assert_eq!(first.context, Some(24_875));
    assert_eq!((first.input, first.output), (24_768, 107));
    assert_eq!(first.cost_usd, None);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, DriveEvent::Unknown { .. })),
        "the record has no line the decoder does not know"
    );
}

#[test]
fn th102_a_codex_plan_gives_each_step_with_its_state() {
    let line = r#"{"method":"turn/plan/updated","params":{"threadId":"t","turnId":"u","explanation":"First the tests","plan":[{"step":"Read the code","status":"completed"},{"step":"Change it","status":"inProgress"},{"step":"Run the tests","status":"pending"},{"step":"Other","status":"new"}]}}"#;
    let events = codex(line);
    let [DriveEvent::Plan { explanation, steps }] = events.as_slice() else {
        panic!("one plan: {events:?}");
    };
    assert_eq!(explanation.as_deref(), Some("First the tests"));
    let states: Vec<StepState> = steps.iter().map(|step| step.state).collect();
    assert_eq!(
        states,
        [
            StepState::Completed,
            StepState::InProgress,
            StepState::Pending,
            StepState::Pending
        ],
        "a state that is not known is pending"
    );
    assert_eq!(steps[1].step, "Change it");
}

#[test]
fn th104_the_codex_models_leave_out_a_hidden_model() {
    let line = r#"{"id":9,"result":{"data":[{"id":"a","model":"gpt-6","displayName":"GPT-6","hidden":false,"supportedReasoningEfforts":[{"reasoningEffort":"low","description":"x"},{"reasoningEffort":"high","description":"y"}],"defaultReasoningEffort":"high"},{"id":"b","model":"secret","displayName":"Hidden","hidden":true,"supportedReasoningEfforts":[],"defaultReasoningEffort":"low"}],"nextCursor":null}}"#;
    let events = codex(line);
    assert!(matches!(events[0], DriveEvent::Reply { ok: true, .. }));
    let [_, DriveEvent::Models { models }] = events.as_slice() else {
        panic!("a reply and the models: {events:?}");
    };
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].value, "gpt-6");
    assert_eq!(models[0].efforts, ["low", "high"]);
    assert_eq!(models[0].default_effort.as_deref(), Some("high"));
}

#[test]
fn th103_a_recorded_claude_turn_gives_its_use_before_its_end() {
    let events = claude(CLAUDE_FULL);
    let at = events
        .iter()
        .position(|event| matches!(event, DriveEvent::Usage { .. }))
        .expect("a use");
    assert!(matches!(events[at + 1], DriveEvent::TurnEnded { .. }));
    let usage = usage_of(&events)[0];
    assert_eq!(usage.window, Some(1_000_000));
    assert_eq!(usage.cost_usd, Some(0.093_456_7));
    assert_eq!(usage.input, 12 + 162_347 + 17_542);
    assert_eq!(usage.output, 703);
    assert_eq!(
        usage.context,
        Some(2 + 30_224 + 166 + 131),
        "the last request"
    );
}

#[test]
fn th104_thr_051_the_claude_models_come_from_initialize_and_its_account_is_dropped() {
    let line = r#"{"type":"control_response","response":{"subtype":"success","request_id":"anima-initialize","response":{"models":[{"value":"opus","displayName":"Opus","supportsEffort":true,"supportedEffortLevels":["low","max"]},{"value":"haiku","displayName":"Haiku","supportsEffort":false,"supportedEffortLevels":["low"]}],"account":{"email":"someone@example.com"}}}}"#;
    let events = claude(line);
    let [
        DriveEvent::Reply { result, .. },
        DriveEvent::Models { models },
    ] = events.as_slice()
    else {
        panic!("a reply and the models: {events:?}");
    };
    assert_eq!(*result, Value::Null, "nothing else of the answer is kept");
    assert!(!format!("{events:?}").contains("someone@example.com"));
    assert_eq!(models[0].efforts, ["low", "max"]);
    assert!(
        models[1].efforts.is_empty(),
        "a model with no effort takes none"
    );
    let interrupt = r#"{"type":"control_response","response":{"subtype":"success","request_id":"anima-interrupt-1","response":{}}}"#;
    assert_eq!(
        claude(interrupt).len(),
        1,
        "an answer with no models gives only the reply"
    );
}

#[test]
fn th104_the_claude_initialize_line_asks_with_no_hooks() {
    let sent: Value =
        serde_json::from_str(&ClaudeInput::initialize("anima-initialize")).expect("json");
    assert_eq!(sent["request"]["subtype"], "initialize");
    assert_eq!(sent["request_id"], "anima-initialize");
    assert!(sent["request"]["hooks"].is_null());
}

#[test]
fn th117_a_codex_model_gives_its_speeds_its_flags_and_the_words_of_its_efforts() {
    let line = r#"{"id":9,"result":{"data":[{"id":"a","model":"gpt-6","displayName":"GPT-6","hidden":false,"isDefault":true,"supportedReasoningEfforts":[{"reasoningEffort":"none","description":"No\u202ereasoning"},{"reasoningEffort":"minimal","description":"Least\nthinking"}],"defaultReasoningEffort":"minimal","serviceTiers":[{"id":"fast","name":"Fast","description":"Quicker"},{"id":"-x","name":"Bad"},{"id":"ultra fast","name":"Bad"}],"defaultServiceTier":"fast","availableAccessPrograms":{"cyber":["standard","daybreakBlue","bad name"]},"availabilityNux":{"message":"New in Codex"},"upgrade":null},{"id":"b","model":"gpt-5","displayName":"GPT-5","hidden":false,"supportedReasoningEfforts":[],"defaultReasoningEffort":"low","upgrade":"gpt-6","availabilityNux":null}],"nextCursor":null}}"#;
    let events = codex(line);
    let [_, DriveEvent::Models { models }] = events.as_slice() else {
        panic!("a reply and the models: {events:?}");
    };
    let new = &models[0];
    assert_eq!(new.efforts, ["none", "minimal"]);
    assert_eq!(
        new.effort_words
            .iter()
            .map(|words| words.description.as_str())
            .collect::<Vec<_>>(),
        ["No reasoning", "Least thinking"],
        "one plain line: no direction mark, no line break"
    );
    assert!(new.default && new.new && !new.legacy);
    assert_eq!(
        new.speeds.len(),
        1,
        "a speed whose id is not a key is left out"
    );
    assert_eq!(new.speeds[0].id, "fast");
    assert_eq!(new.default_speed.as_deref(), Some("fast"));
    assert_eq!(new.access_programs, ["standard", "daybreakBlue"]);
    let old = &models[1];
    assert!(old.legacy && !old.new && !old.default);
    assert_eq!(old.upgrade.as_deref(), Some("gpt-6"));
    assert!(old.speeds.is_empty() && old.access_programs.is_empty());
}

#[test]
fn th117_a_claude_model_has_no_flag_that_claude_code_does_not_give() {
    let line = r#"{"type":"control_response","response":{"subtype":"success","request_id":"anima-initialize","response":{"models":[{"value":"opus","displayName":"Opus","supportsEffort":true,"supportedEffortLevels":["low"],"supportsFastMode":true}]}}}"#;
    let events = claude(line);
    let [_, DriveEvent::Models { models }] = events.as_slice() else {
        panic!("a reply and the models: {events:?}");
    };
    assert!(
        models[0].speeds.is_empty(),
        "fast mode of Claude Code has no flag that the runner may give"
    );
    let shown = serde_json::to_value(&models[0]).expect("json");
    assert_eq!(
        shown.as_object().expect("an object").len(),
        4,
        "an empty extra is left out of the answer: {shown}"
    );
}
