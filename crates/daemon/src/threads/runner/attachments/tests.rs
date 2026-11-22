use std::time::{Duration, Instant};

use agent_threads::control::Refusal;
use agent_threads::drive::{Attached, Program};
use base64::Engine as _;
use uuid::Uuid;

use super::{Attachments, PENDING_FOR};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n-the-rest-of-a-png";

fn encoded(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn attached(store: &mut Attachments, thread: Uuid, name: &str, mime: &str, bytes: &[u8]) -> Uuid {
    let id = store
        .begin(thread, name, mime, bytes.len() as u64)
        .expect("a new file");
    assert_eq!(
        store.add(thread, id, &encoded(bytes)),
        Ok(bytes.len() as u64)
    );
    id
}

#[test]
fn th111_a_file_in_parts_goes_with_a_prompt_where_the_program_reads_it() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let thread = Uuid::now_v7();
    let id = store
        .begin(thread, "../notes.txt", "text/plain", 6)
        .expect("a new file");
    assert_eq!(store.add(thread, id, &encoded(b"abc")), Ok(3));
    assert_eq!(store.add(thread, id, &encoded(b"def")), Ok(6));
    let taken = store
        .take(thread, &[id], Program::Codex)
        .expect("the files");
    assert_eq!(taken[0].name, "notes.txt");
    assert_eq!(taken[0].kind, Attached::File);
    assert!(taken[0].path.starts_with(folder.path().join("attachments")));
    assert_eq!(std::fs::read(&taken[0].path).expect("the file"), b"abcdef");
    assert!(
        store.take(thread, &[id], Program::Codex).is_err(),
        "a file goes once"
    );
}

#[test]
fn th111_an_image_goes_inline_to_claude_code_and_by_its_path_to_codex() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let thread = Uuid::now_v7();
    let for_claude = attached(&mut store, thread, "a.png", "image/png", PNG);
    let for_codex = attached(&mut store, thread, "b.png", "image/png", PNG);
    let claude = store
        .take(thread, &[for_claude], Program::ClaudeCode)
        .expect("the image");
    assert_eq!(claude[0].kind.inline(), Some(encoded(PNG).as_str()));
    let codex = store
        .take(thread, &[for_codex], Program::Codex)
        .expect("the image");
    assert!(matches!(
        codex[0].kind,
        Attached::Image { inline: None, .. }
    ));
}

#[test]
fn th111_an_image_whose_bytes_are_not_its_type_is_refused_and_removed() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let thread = Uuid::now_v7();
    let id = store
        .begin(thread, "x.png", "image/png", 5)
        .expect("a new file");
    assert!(matches!(
        store.add(thread, id, &encoded(b"<svg>")),
        Err(Refusal::CannotAttach { said }) if said.contains("x.png")
    ));
    assert!(store.take(thread, &[id], Program::Codex).is_err());
}

#[test]
fn th111_the_limits_are_checked_again() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let thread = Uuid::now_v7();
    assert!(matches!(
        store.begin(thread, "big.png", "image/png", 11 * 1024 * 1024),
        Err(Refusal::TooLarge { said }) if said.contains("big.png")
    ));
    assert!(store.begin(thread, "empty", "text/plain", 0).is_err());
    let id = store
        .begin(thread, "a.txt", "text/plain", 2)
        .expect("a new file");
    assert!(
        matches!(
            store.add(thread, id, &encoded(b"abc")),
            Err(Refusal::TooLarge { .. })
        ),
        "more bytes than the size"
    );
    let half = store
        .begin(thread, "b.txt", "text/plain", 4)
        .expect("a new file");
    store.add(thread, half, &encoded(b"ab")).expect("a part");
    assert!(
        store.take(thread, &[half], Program::Codex).is_err(),
        "a file that is not complete does not go"
    );
    assert_eq!(
        store.add(thread, half, "not base64!"),
        Err(Refusal::BadValue)
    );
}

#[test]
fn th111_a_file_is_only_of_its_own_thread() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let (mine, other) = (Uuid::now_v7(), Uuid::now_v7());
    let id = attached(&mut store, mine, "a.txt", "text/plain", b"ab");
    assert!(store.add(other, id, &encoded(b"c")).is_err());
    assert!(store.drop_file(other, id).is_err());
    assert!(store.take(other, &[id], Program::Codex).is_err());
    assert!(
        store.take(mine, &[id, id], Program::Codex).is_err(),
        "named twice"
    );
    assert!(store.take(mine, &[id], Program::Codex).is_ok());
}

#[test]
fn th111_a_file_that_did_not_go_is_removed_after_an_hour_and_at_a_start() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let thread = Uuid::now_v7();
    let old = attached(&mut store, thread, "a.txt", "text/plain", b"ab");
    store.expire(Instant::now() + PENDING_FOR + Duration::from_secs(1));
    assert!(store.take(thread, &[old], Program::Codex).is_err());
    let kept = attached(&mut store, thread, "b.txt", "text/plain", b"ab");
    let pending = folder
        .path()
        .join("attachments")
        .join(thread.to_string())
        .join("pending");
    assert!(pending.join(kept.to_string()).exists());
    let _again = Attachments::at(folder.path());
    assert!(
        !pending.exists(),
        "a new runner knows of no file that waited"
    );
}

#[test]
fn th111_the_files_of_a_removed_thread_are_removed() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let thread = Uuid::now_v7();
    let id = attached(&mut store, thread, "a.txt", "text/plain", b"ab");
    let sent = store.take(thread, &[id], Program::Codex).expect("the file");
    store.forget(thread);
    assert!(!sent[0].path.exists());
}
