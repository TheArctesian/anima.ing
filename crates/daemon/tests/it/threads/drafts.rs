//! Files attached on the page of a new thread, before the thread exists (TH111): they wait under
//! the key of the draft and go with the first prompt of the start that names it.

use agent_threads::control::{Action, Change, Outcome, Refusal};
use agent_threads::drive::{Mode, Program};
use uuid::Uuid;

use super::attach::{CLAUDE_KEEPS, encoded, last_file, prompt_line};
use super::runner::{Kit, change};

fn start(kit: &Kit, draft: Uuid, attachments: Vec<Uuid>) -> Action {
    Action::Start {
        program: Program::ClaudeCode,
        folder: kit.path("work"),
        mode: Mode::FullAccess,
        model: None,
        effort: None,
        prompt: "look".to_owned(),
        program_path: None,
        draft: Some(draft),
        attachments,
    }
}

fn draft_file(thread: Uuid, bytes: &[u8]) -> Action {
    Action::Attach {
        thread,
        name: "notes.txt".to_owned(),
        mime: "text/plain".to_owned(),
        size: bytes.len() as u64,
        bytes: encoded(bytes),
        draft: true,
    }
}

#[tokio::test]
async fn th111_a_file_of_the_draft_goes_with_the_first_prompt_of_the_start_that_names_it() {
    let kit = Kit::with(CLAUDE_KEEPS);
    std::fs::create_dir_all(kit.path("work")).expect("a folder");
    let mut client = kit.client().await;
    client.call(Action::Watch).await;
    let draft = Uuid::now_v7();
    let call = client.call(draft_file(draft, b"hello")).await;
    let Outcome::Attached { id: notes, .. } = client.done(call).await else {
        panic!("the file of the draft came");
    };

    let other = client.call(start(&kit, Uuid::now_v7(), vec![notes])).await;
    assert!(
        matches!(
            client.done(other).await,
            Outcome::Refused {
                why: Refusal::CannotAttach { .. }
            }
        ),
        "a file goes only with the start of its draft, and nothing starts"
    );

    let call = client.call(start(&kit, draft, vec![notes])).await;
    let shown = client
        .until(|reply| matches!(change(reply), Some(Change::Prompt { text, .. }) if text == "look"))
        .await;
    let Outcome::Started { thread } = client.done(call).await else {
        panic!("the thread started");
    };
    let Some(Change::Prompt { files, .. }) = change(&shown) else {
        unreachable!("only a prompt is wanted");
    };
    assert_eq!(files, &["notes.txt".to_owned()]);
    let first = prompt_line(&kit, 1).await;
    let words = first["message"]["content"][0]["text"]
        .as_str()
        .expect("words");
    let named = last_file(
        words,
        &kit.threads().join("attachments").join(thread.to_string()),
    );
    assert_eq!(std::fs::read(named).expect("the file"), b"hello");
    assert!(
        !kit.threads()
            .join("attachments")
            .join(draft.to_string())
            .exists(),
        "the draft keeps no folder"
    );

    let taken = client.call(draft_file(thread, b"a")).await;
    assert_eq!(
        client.done(taken).await,
        Outcome::Refused {
            why: Refusal::BadValue
        },
        "the ID of a thread is not a key of a draft"
    );
}
