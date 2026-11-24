use super::*;
use crate::drive::{Answer, DriveEvent, Launch, Mode, Program, SessionStart};
use serde_json::json;

#[test]
fn th119_only_the_one_time_permission_option_can_be_allowed() {
    let frame = json!({"jsonrpc":"2.0","id":"permission-1","method":"session/request_permission","params":{
        "toolCall":{"title":"Write file","kind":"edit","rawInput":{"path":"file.txt","content":"hello"}},
        "options":[{"optionId":"once-7","kind":"allow_once"},{"optionId":"forever-8","kind":"allow_always"}]
    }});
    let DriveEvent::Request(request) = read(frame.to_string().as_bytes()).remove(0) else {
        panic!("permission")
    };
    let allowed: serde_json::Value =
        serde_json::from_str(&answer(&request, &Answer::Allow).expect("allow once")).expect("JSON");
    assert_eq!(
        allowed["result"]["outcome"],
        json!({"outcome":"selected","optionId":"once-7"})
    );
    assert!(answer(&request, &Answer::AllowForSession { rule: 0 }).is_err());
    let denied: serde_json::Value = serde_json::from_str(
        &answer(
            &request,
            &Answer::Deny {
                message: "No".into(),
            },
        )
        .expect("deny"),
    )
    .expect("JSON");
    assert_eq!(denied["result"]["outcome"]["outcome"], "cancelled");
}

#[test]
fn th119_unknown_client_requests_fail_closed_and_bad_frames_never_execute() {
    for line in [
        r#"{"jsonrpc":"2.0","id":9,"method":"fs/write_text_file","params":{"path":"/tmp/no"}}"#,
        "invalid",
    ] {
        assert!(matches!(
            read(line.as_bytes()).as_slice(),
            [DriveEvent::Unknown {
                stops_turn: true,
                ..
            }]
        ));
    }
    let launch = |mode| Launch {
        program: Program::Grok,
        path: "/usr/bin/grok".into(),
        mode,
        model: None,
        effort: None,
        session: SessionStart::New(uuid::Uuid::nil()),
    };
    assert!(launch(Mode::Plan).invocation().is_err());
    assert!(launch(Mode::AcceptEdits).invocation().is_err());
    let params: serde_json::Value =
        serde_json::from_str(&new_session("/work", Mode::Ask)).expect("JSON");
    assert_eq!(
        params["params"]["_meta"],
        json!({"yoloMode":false,"autoMode":false})
    );
}

#[test]
fn th119_models_and_effort_are_discovered_without_inventing_choices() {
    let state = json!({"currentModelId":"grok-future","availableModels":[
        {"modelId":"grok-future","name":"Future","_meta":{"reasoningEffort":"high","reasoningEfforts":[{"id":"high"},{"id":"unknown-effort"}]}},
        {"modelId":"--bad-flag","name":"Bad"}
    ]});
    let Some(DriveEvent::Models { models }) = models(&state) else {
        panic!("models")
    };
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].efforts, ["high"]);
    assert_eq!(models[0].default_effort.as_deref(), Some("high"));
    assert!(models[0].default);
}

#[test]
fn th119_ambiguous_permission_identifiers_are_refused() {
    let frame = json!({"jsonrpc":"2.0","id":1,"method":"session/request_permission","params":{
        "toolCall":{"title":"tool"}, "options":[
            {"optionId":"same","kind":"allow_once"},{"optionId":"same","kind":"allow_always"}
        ]
    }});
    assert!(matches!(
        read(frame.to_string().as_bytes()).as_slice(),
        [DriveEvent::Unknown {
            stops_turn: true,
            ..
        }]
    ));
}
