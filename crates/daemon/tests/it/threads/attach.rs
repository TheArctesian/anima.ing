//! Files with a prompt (TH111): the person attaches a file in parts, sends it with a prompt, and
//! the program gets an image inside its line and each other file by its path.

use agent_threads::control::{Action, Change, Outcome, Refusal, RunState};
use agent_threads::drive::{Mode, Program};
use base64::Engine as _;
use uuid::Uuid;

use super::runner::{Client, Kit, change, state_is};

/// A made Claude Code that writes each prompt line it gets to `prompts.jsonl`, then ends the turn.
pub(super) const CLAUDE_KEEPS: &str = r#"#!/usr/bin/perl
$| = 1;
my $sid = "none";
for my $i (0 .. $#ARGV) { $sid = $ARGV[$i + 1] if $ARGV[$i] eq "--session-id"; }
print qq({"type":"system","subtype":"init","session_id":"$sid","model":"made","permissionMode":"bypassPermissions"}\n);
while (my $line = <STDIN>) {
    if ($line =~ /"type":"user"/) {
        open(my $file, ">>", "prompts.jsonl"); print $file $line; close($file);
        print qq({"type":"system","subtype":"session_state_changed","state":"running"}\n);
        print qq({"type":"result","subtype":"success","is_error":false,"result":"done","terminal_reason":"completed"}\n);
        print qq({"type":"system","subtype":"session_state_changed","state":"idle"}\n);
    }
}
"#;

/// The first bytes of a PNG of 64 by 64 pixels.
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x40\0\0\0\x40-the-rest";

pub(super) fn encoded(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

async fn idle_thread(kit: &Kit, client: &mut Client) -> Uuid {
    client.call(Action::Watch).await;
    let start = client
        .call(Action::Start {
            program: Program::ClaudeCode,
            folder: kit.path("work"),
            mode: Mode::FullAccess,
            model: None,
            effort: None,
            speed: None,
            prompt: "first".to_owned(),
            program_path: None,
            draft: None,
            attachments: Vec::new(),
            worktree: false,
            base: None,
        })
        .await;
    let Outcome::Started { thread, .. } = client.done(start).await else {
        panic!("the thread started");
    };
    client.until(state_is(RunState::Idle)).await;
    thread
}

/// Attaches a whole file in one part; its MIME type comes from its name.
async fn attach(client: &mut Client, thread: Uuid, name: &str, bytes: &[u8]) -> Outcome {
    let mime = if std::path::Path::new(name)
        .extension()
        .is_some_and(|e| e == "png")
    {
        "image/png"
    } else {
        "text/plain"
    };
    let call = client
        .call(Action::Attach {
            thread,
            name: name.to_owned(),
            mime: mime.to_owned(),
            size: bytes.len() as u64,
            bytes: encoded(bytes),
            draft: false,
        })
        .await;
    client.done(call).await
}

/// The `count`th prompt line that the made program got.
pub(super) async fn prompt_line(kit: &Kit, count: usize) -> serde_json::Value {
    for _ in 0..200 {
        let lines =
            std::fs::read_to_string(kit.path("work").join("prompts.jsonl")).unwrap_or_default();
        if let Some(line) = lines.lines().nth(count - 1) {
            return serde_json::from_str(line).expect("json");
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("the made program got no prompt {count}");
}

/// The path of the last file that the words name: below `attachments`, a stored name with no
/// text of the person, then the name of the person as a JSON string.
pub(super) fn last_file<'a>(words: &'a str, attachments: &std::path::Path) -> &'a str {
    let (path, name) = words
        .rsplit("\n- ")
        .next()
        .and_then(|line| line.split_once(" (named "))
        .expect("a path and its name");
    assert_eq!(name, "\"notes.txt\")", "{words}");
    let path = std::path::Path::new(path);
    assert!(path.starts_with(attachments), "{words}");
    assert!(path.extension().is_some_and(|e| e == "txt"), "{words}");
    assert!(!path.to_string_lossy().contains("notes"), "{words}");
    path.to_str().expect("a path")
}

/// Attaches [`PNG`] in two parts, as a large file goes.
async fn attach_in_two_parts(client: &mut Client, thread: Uuid) -> Uuid {
    let (head, rest) = PNG.split_at(5);
    let first = client
        .call(Action::Attach {
            thread,
            name: "shot.png".to_owned(),
            mime: "image/png".to_owned(),
            size: PNG.len() as u64,
            bytes: encoded(head),
            draft: false,
        })
        .await;
    let Outcome::Attached {
        id: image,
        received: 5,
    } = client.done(first).await
    else {
        panic!("the first part came");
    };
    let more = client
        .call(Action::AttachMore {
            thread,
            attachment: image,
            bytes: encoded(rest),
        })
        .await;
    assert_eq!(
        client.done(more).await,
        Outcome::Attached {
            id: image,
            received: PNG.len() as u64
        }
    );
    image
}

#[tokio::test]
async fn th111_an_image_goes_inside_the_line_and_a_file_goes_by_its_path() {
    let kit = Kit::with(CLAUDE_KEEPS);
    std::fs::create_dir_all(kit.path("work")).expect("a folder");
    let mut client = kit.client().await;
    let thread = idle_thread(&kit, &mut client).await;
    let image = attach_in_two_parts(&mut client, thread).await;
    let Outcome::Attached { id: notes, .. } =
        attach(&mut client, thread, "../notes.txt", b"hello").await
    else {
        panic!("the file came");
    };
    let send = client
        .call(Action::Send {
            thread,
            text: "look".to_owned(),
            attachments: vec![image, notes],
            skills: Vec::new(),
        })
        .await;
    // The prompt is shown when the holder took its line, before the call is answered.
    let shown = client
        .until(|reply| matches!(change(reply), Some(Change::Prompt { text, .. }) if text == "look"))
        .await;
    assert_eq!(client.done(send).await, Outcome::Ok);
    let Some(Change::Prompt { images, files, .. }) = change(&shown) else {
        unreachable!("only a prompt is wanted");
    };
    assert_eq!(
        (*images, files.as_slice()),
        (1, ["notes.txt".to_owned()].as_slice())
    );

    let last = prompt_line(&kit, 2).await;
    let content = &last["message"]["content"];
    let words = content[0]["text"].as_str().expect("words");
    let named = last_file(
        words,
        &kit.threads().join("attachments").join(thread.to_string()),
    );
    assert_eq!(content[1]["type"], "image");
    assert_eq!(content[1]["source"]["data"], encoded(PNG));
    assert_eq!(std::fs::read(named).expect("the file"), b"hello");

    let again = client
        .call(Action::Send {
            thread,
            text: "again".to_owned(),
            attachments: vec![notes],
            skills: Vec::new(),
        })
        .await;
    assert!(
        matches!(
            client.done(again).await,
            Outcome::Refused {
                why: Refusal::CannotAttach { .. }
            }
        ),
        "a file goes once"
    );
}

#[tokio::test]
async fn th111_an_image_that_is_not_its_type_and_a_file_over_the_limit_are_refused_with_words() {
    let kit = Kit::with(CLAUDE_KEEPS);
    std::fs::create_dir_all(kit.path("work")).expect("a folder");
    let mut client = kit.client().await;
    let thread = idle_thread(&kit, &mut client).await;
    assert!(matches!(
        attach(&mut client, thread, "fake.png", b"<svg/>").await,
        Outcome::Refused { why: Refusal::CannotAttach { said } } if said.contains("fake.png")
    ));
    let big = client
        .call(Action::Attach {
            thread,
            name: "huge.mov".to_owned(),
            mime: "video/quicktime".to_owned(),
            size: 51 * 1024 * 1024,
            bytes: String::new(),
            draft: false,
        })
        .await;
    assert!(matches!(
        client.done(big).await,
        Outcome::Refused { why: Refusal::TooLarge { said } } if said.contains("huge.mov")
    ));
    let unknown = client
        .call(Action::Attach {
            thread: Uuid::now_v7(),
            name: "a.txt".to_owned(),
            mime: "text/plain".to_owned(),
            size: 1,
            bytes: encoded(b"a"),
            draft: false,
        })
        .await;
    assert_eq!(
        client.done(unknown).await,
        Outcome::Refused {
            why: Refusal::UnknownThread
        }
    );
}
