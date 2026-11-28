use super::*;

#[test]
fn th116_a_git_answer_reads_back_from_its_frame() {
    let done = Reply::Done {
        id: 42,
        result: Outcome::Git {
            result: GitOutcome::Reverted {
                turn: 2,
                undo: "b2".to_owned(),
            },
        },
    };
    let frame = serde_json::to_string(&done).expect("a frame");
    let read: Reply =
        serde_json::from_str(&frame).unwrap_or_else(|error| panic!("{frame}: {error}"));
    assert_eq!(read, done, "{frame}");
}

#[test]
fn tc7_git_legacy_removal_frames_are_accepted_but_never_sent_to_older_runners() {
    let old_frame = serde_json::json!({
        "id": 4,
        "call": "git",
        "thread": "c30b92dd-5d0d-40e2-978e-3f26d0cf9af1",
        "repository": "fixture",
        "git": "remove"
    });
    let read: Call = serde_json::from_value(old_frame).expect("legacy request");
    assert!(matches!(
        read.action,
        Action::Git {
            action: GitAction::Remove,
            ..
        }
    ));
    assert_eq!(
        serde_json::to_value(read).expect("new request")["call"],
        "git_v2"
    );
}

#[test]
fn tc7_w3_the_git_read_frame_refuses_caller_selected_identity() {
    for selector in ["thread", "device", "credential"] {
        let mut frame = serde_json::json!({
            "id": 1, "call": "git_read", "repository": "work", "read": {"operation":"status"}
        });
        frame[selector] = serde_json::json!("fixture-selector");
        assert!(serde_json::from_value::<Call>(frame).is_err(), "{selector}");
    }
}
