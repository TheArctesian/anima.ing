//! A Codex thread in a folder that holds the folder of Anima keeps that folder read-only while its
//! mode is in the folder sandbox (THR-041).

use std::path::PathBuf;

use super::*;

#[test]
fn thr_041_a_codex_thread_in_the_folder_sandbox_keeps_the_folder_of_anima_read_only() {
    let folder = PathBuf::from("/home/you");
    let anima = KeptFolder::new(
        PathBuf::from("/home/you/Library/anima-daemon"),
        uuid::Uuid::from_u128(7),
    );
    let place = CodexPlace {
        folder: &folder,
        kept: Some(&anima),
    };
    let start: serde_json::Value =
        serde_json::from_str(&CodexInput::thread_start(1, place, Mode::Ask, None)).expect("JSON");
    let params = &start["params"];
    assert!(
        params.get("sandbox").is_none(),
        "Codex takes the profile only when the start names no sandbox"
    );
    let name = "anima-keeps-00000000000000000000000000000007";
    let profile = &params["config"]["permissions"][name];
    assert_eq!(
        params["config"]["default_permissions"], name,
        "a name of the thread, which no file of configuration can hold before the start"
    );
    assert_eq!(profile["extends"], ":workspace");
    assert_eq!(
        profile["filesystem"]["/home/you/Library/anima-daemon"],
        "read"
    );

    let full: serde_json::Value =
        serde_json::from_str(&CodexInput::thread_start(1, place, Mode::FullAccess, None))
            .expect("JSON");
    assert_eq!(full["params"]["sandbox"], "danger-full-access");
    assert!(
        full["params"].get("config").is_none(),
        "Full access has no sandbox"
    );

    let turn = |mode, keeps| -> serde_json::Value {
        let settings = TurnSettings {
            mode: Some(mode),
            keeps,
            ..TurnSettings::default()
        };
        serde_json::from_str(&CodexInput::turn_start(
            2,
            "t",
            &Prompt::plain("go"),
            &settings,
        ))
        .expect("JSON")
    };
    let kept = turn(Mode::AcceptEdits, true);
    assert!(
        kept["params"].get("sandboxPolicy").is_none(),
        "the profile stays"
    );
    assert_eq!(kept["params"]["approvalPolicy"], "on-request");
    assert_eq!(
        turn(Mode::FullAccess, true)["params"]["sandboxPolicy"]["type"],
        "dangerFullAccess"
    );
}
