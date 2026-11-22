//! The protocol of `codex app-server`: TH10 Section 3.2 and the TH18 runs.

use std::path::PathBuf;

use serde_json::json;

use crate::control::attach::ImageKind;

use super::*;

#[test]
fn a_codex_turn_in_full_access_gives_calls_outputs_and_the_end() {
    let events = codex(CODEX_FULL);
    assert!(!stops(&events));
    assert_eq!(requests(&events).len(), 0);
    assert_eq!(ends(&events), [TurnOutcome::Completed]);
    let calls: Vec<&str> = events
        .iter()
        .filter_map(|event| match event {
            DriveEvent::ToolCall { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(calls, ["command", "patch", "command", "command"]);
    assert!(events.iter().any(
        |event| matches!(event, DriveEvent::Session { id, .. } if id.starts_with("01a1fbaf"))
    ));
    let text = serde_json::to_string(&events).expect("events serialise");
    assert!(
        !text.contains("ACCOUNT-FIELD"),
        "no account member reaches an event"
    );
}

const CODEX_APPROVAL: &str = r#"{"method":"item/commandExecution/requestApproval","id":1,"params":{"kind":"command","threadId":"t","turnId":"u","itemId":"exec-1","command":"/bin/zsh -lc 'mkdir x3a'","cwd":"/w","proposedExecpolicyAmendment":["mkdir","x3a"],"availableDecisions":["accept","acceptForSession",{"acceptWithExecpolicyAmendment":{"execpolicy_amendment":["mkdir","x3a"]}},"cancel"]}}"#;

#[test]
fn a_codex_approval_offers_only_the_rules_of_the_request() {
    let request = one_request(codex(CODEX_APPROVAL));
    assert_eq!(request.kind, RequestKind::Command);
    assert_eq!(request.subject, "/bin/zsh -lc 'mkdir x3a'");
    assert_eq!(request.offered.len(), 1);
    let accept = line(&CodexInput::answer(&request, &Answer::Allow).expect("fits"));
    assert_eq!(
        accept,
        json!({"jsonrpc": "2.0", "id": 1, "result": {"decision": "accept"}})
    );
    let decline = line(
        &CodexInput::answer(
            &request,
            &Answer::Deny {
                message: String::new(),
            },
        )
        .expect("fits"),
    );
    assert_eq!(decline["result"]["decision"], "decline");
    let session =
        line(&CodexInput::answer(&request, &Answer::AllowForSession { rule: 0 }).expect("fits"));
    assert_eq!(
        session["result"]["decision"], "acceptForSession",
        "the amendment of the exec policy, a lasting rule, is not offered"
    );
    assert!(CodexInput::answer(&request, &Answer::AllowForSession { rule: 1 }).is_err());
}

#[test]
fn a_codex_file_approval_shows_the_diff_of_its_item() {
    let lines = [
        r#"{"method":"item/started","params":{"item":{"type":"fileChange","id":"exec-ff","changes":[{"path":"/w/x3.txt","kind":{"type":"add"},"diff":"ok\n"}],"status":"inProgress"}}}"#,
        r#"{"method":"item/fileChange/requestApproval","id":3,"params":{"threadId":"t","turnId":"u","itemId":"exec-ff","reason":null,"grantRoot":null}}"#,
    ];
    let request = one_request(codex(&lines.join("\n")).into_iter().skip(1).collect());
    assert_eq!(request.kind, RequestKind::FileChange);
    assert_eq!(request.diff.as_deref(), Some("/w/x3.txt\nok\n"));
}

#[test]
fn a_codex_interrupt_closes_the_calls_that_are_still_open() {
    let lines = [
        r#"{"method":"item/started","params":{"item":{"type":"commandExecution","id":"exec-9","command":"sleep 31","status":"inProgress"}}}"#,
        r#"{"method":"turn/completed","params":{"threadId":"t","turn":{"id":"u","items":[],"status":"interrupted","error":null}}}"#,
    ];
    let events = codex(&lines.join("\n"));
    assert!(
        matches!(&events[1], DriveEvent::ToolOutput { id, failed: true, .. } if id == "exec-9")
    );
    assert_eq!(ends(&events), [TurnOutcome::Interrupted]);
}

#[test]
fn a_codex_question_is_answered_by_the_id_of_each_question() {
    let ask = r#"{"method":"item/tool/requestUserInput","id":"q1","params":{"itemId":"i","questions":[{"id":"colour","header":"Colour","question":"Red or blue?","options":[{"label":"Red"},{"label":"Blue"}]}],"isBlocking":true}}"#;
    let request = one_request(codex(ask));
    assert_eq!(request.kind, RequestKind::Question);
    let answer = Answer::Choose {
        answers: vec![("colour".into(), "Blue".into())],
    };
    let sent = line(&CodexInput::answer(&request, &answer).expect("fits"));
    assert_eq!(sent["id"], "q1");
    assert_eq!(sent["result"]["answers"]["colour"]["answers"][0], "Blue");
}

#[test]
fn a_codex_request_of_an_unknown_kind_stops_the_turn_and_is_refused() {
    let events = codex(r#"{"method":"account/chatgptAuthTokens/refresh","id":7,"params":{}}"#);
    assert!(stops(&events));
    let refusal = line(&CodexInput::refuse(
        &ProgramRequestId::Number(7),
        "not in the list",
    ));
    assert_eq!(refusal["id"], 7);
    assert_eq!(refusal["error"]["code"], -32601);
    assert!(
        !stops(&codex(r#"{"method":"thread/somethingNew","params":{}}"#)),
        "an unknown notice is shown, not fatal"
    );
    assert_eq!(
        codex(r#"{"method":"account/rateLimits/updated","params":{"x":1}}"#),
        []
    );
}

#[test]
fn each_codex_line_the_runner_can_write_names_a_method_of_the_closed_list() {
    let folder = PathBuf::from("/w");
    let place = CodexPlace {
        folder: &folder,
        kept: None,
    };
    let turn = CodexTurn {
        thread: "t",
        turn: "u",
    };
    let written = [
        CodexInput::initialize(1),
        CodexInput::initialized(),
        CodexInput::thread_start(2, place, Mode::FullAccess, None),
        CodexInput::thread_resume(3, "t", &folder, Mode::Ask),
        CodexInput::thread_fork(4, "t", &folder, Mode::Ask),
        CodexInput::turn_start(
            5,
            "t",
            &Prompt::plain("hello"),
            &TurnSettings {
                effort: Some(Effort::Low),
                ..TurnSettings::default()
            },
        ),
        CodexInput::turn_steer(6, turn, &Prompt::plain("also")),
        CodexInput::turn_interrupt(7, turn),
        CodexInput::account_read(8),
        CodexInput::model_list(9),
    ];
    let names: Vec<&str> = CodexMethod::ALL
        .iter()
        .map(|method| method.name())
        .collect();
    for text in &written {
        let method = line(text)["method"].as_str().expect("a method").to_owned();
        assert!(
            names.contains(&method.as_str()),
            "{method} is not in the closed list"
        );
    }
    assert!(
        names
            .iter()
            .all(|name| !name.to_lowercase().contains("token"))
    );
    let start = line(&written[2]);
    assert_eq!(start["params"]["sandbox"], "danger-full-access");
    assert_eq!(start["params"]["approvalPolicy"], "never");
}

#[test]
fn thr_051_a_codex_reply_keeps_only_the_ids_of_the_thread_and_the_turn() {
    let events = codex(
        r#"{"id":1,"result":{"userAgent":"x","codexHome":"/Users/someone/.codex"}}
{"id":2,"result":{"thread":{"id":"th-1","cwd":"/w","path":"/Users/someone/.codex/s.jsonl"}}}
{"id":3,"result":{"account":{"email":"someone@example.com","planType":"pro"}}}"#,
    );
    let text = serde_json::to_string(&events).expect("events serialise");
    assert!(!text.contains("someone"), "{text}");
    assert!(
        matches!(&events[1], DriveEvent::Reply { result, .. } if result["thread"]["id"] == "th-1")
    );
}

#[test]
fn thr_046_a_request_id_that_does_not_read_stops_the_turn() {
    assert!(stops(&codex(
        r#"{"method":"item/commandExecution/requestApproval","id":1.5,"params":{}}"#
    )));
}

#[test]
fn th110_a_turn_carries_the_model_the_effort_and_the_mode_that_the_person_changed() {
    let model = Model::new("gpt-5.6-sol").expect("a model");
    let changed = line(&CodexInput::turn_start(
        5,
        "t",
        &Prompt::plain("go"),
        &TurnSettings {
            model: Some(&model),
            effort: Some(Effort::Medium),
            mode: Some(Mode::Ask),
            keeps: false,
        },
    ));
    let params = &changed["params"];
    assert_eq!(params["model"], "gpt-5.6-sol");
    assert_eq!(params["effort"], "medium");
    assert_eq!(params["approvalPolicy"], "untrusted");
    assert_eq!(params["sandboxPolicy"], json!({"type": "workspaceWrite"}));
    let kept = line(&CodexInput::turn_start(
        6,
        "t",
        &Prompt::plain("go"),
        &TurnSettings::default(),
    ));
    for field in ["model", "effort", "approvalPolicy", "sandboxPolicy"] {
        assert!(
            kept["params"].get(field).is_none(),
            "{field} is the thread's own"
        );
    }
}

#[test]
fn th111_a_codex_turn_gets_each_image_as_a_local_image_and_each_other_file_by_its_path() {
    let prompt = Prompt {
        text: "look".to_owned(),
        attachments: vec![
            Attachment {
                name: "shot.png".to_owned(),
                path: PathBuf::from("/a/1/shot.png"),
                kind: Attached::Image {
                    kind: ImageKind::Png,
                    inline: Some("AAAA".to_owned()),
                },
            },
            Attachment {
                name: "notes.pdf".to_owned(),
                path: PathBuf::from("/a/2/notes.pdf"),
                kind: Attached::Pdf { inline: None },
            },
        ],
    };
    let turn = line(&CodexInput::turn_start(
        5,
        "t",
        &prompt,
        &TurnSettings::default(),
    ));
    let input = turn["params"]["input"].as_array().expect("an input");
    let words = input[0]["text"].as_str().expect("words");
    assert!(words.starts_with("look\n\n"));
    assert!(words.contains("\n- /a/2/notes.pdf"));
    assert!(!words.contains("shot.png"), "an image goes as an image");
    assert_eq!(
        input[1],
        json!({"type": "localImage", "path": "/a/1/shot.png"})
    );
    assert_eq!(
        input.len(),
        2,
        "Codex reads the image itself: no bytes in the line"
    );
}
