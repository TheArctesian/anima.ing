//! The child fetch process (task L1-6b): the child in its mode, and the daemon side with a
//! stalled and a crashed child.

use std::io::{Read as _, Write as _};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anima_daemon::model::fetch::child::{FETCH_MODE, finish};
use anima_daemon::model::fetch::spawn::fetch_in_child;
use anima_daemon::model::fetch::{Hashed, hex};
use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::releases::fetch::FetchOrder;
use sha2::{Digest as _, Sha256};

use super::kit::{FAKE_RUNTIME, PROGRAM, archive};

const DAEMON: &str = env!("CARGO_BIN_EXE_anima-daemon");

fn order(root: &Path) -> FetchOrder {
    FetchOrder {
        runtime: "llama_server".to_owned(),
        address: "https://github.com/o/r/releases/download/b6000/llama.tar.gz".to_owned(),
        sha256: "0".repeat(64),
        size_bytes: 1 << 20,
        archive_folder: root.to_str().expect("UTF-8").to_owned(),
        unpack_folder: root.join("into").to_str().expect("UTF-8").to_owned(),
        reserve: 0,
        parent: std::process::id(),
    }
}

/// Runs the child with `input` and the environment `variables`, and gives its reply. An input
/// that ends a line stays open until the child ends, as the daemon keeps it.
fn child(input: &[u8], variables: &[(&str, &str)]) -> String {
    let (reply, code) = child_ending(input, variables);
    assert_eq!(code, Some(0));
    reply
}

/// The reply and the exit code of the child.
fn child_ending(input: &[u8], variables: &[(&str, &str)]) -> (String, Option<i32>) {
    let mut child = Command::new(DAEMON)
        .arg(FETCH_MODE)
        .env_clear()
        .envs(variables.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("started");
    let mut stdin = child.stdin.take().expect("stdin");
    // A child that stops reading at its limit closes the pipe; that is not a failure here.
    let _ = stdin.write_all(input);
    // An input with no end of line is closed at once, so the child can read to its end.
    let stdin = input.ends_with(b"\n").then_some(stdin);
    let mut reply = String::new();
    child
        .stdout
        .take()
        .expect("stdout")
        .read_to_string(&mut reply)
        .expect("read");
    let code = child.wait().expect("ended").code();
    drop(stdin);
    (reply, code)
}

#[test]
fn the_child_refuses_to_work_with_any_environment_variable() {
    let root = tempfile::tempdir().expect("a folder");
    let text = line(&order(root.path()));
    assert_eq!(
        child(&text, &[("ANIMA_DAEMON_TOKEN", "x")]),
        "source_failed\n"
    );
    assert_eq!(
        child(&text, &[("HTTPS_PROXY", "http://127.0.0.1:1")]),
        "source_failed\n"
    );
}

#[test]
fn the_child_refuses_an_order_that_does_not_read_or_breaks_a_rule() {
    let root = tempfile::tempdir().expect("a folder");
    assert_eq!(child(b"{", &[]), "runtime_not_configured\n");
    assert_eq!(
        child(&vec![b' '; 70 << 10], &[]),
        "runtime_not_configured\n"
    );
    let mut plain = order(root.path());
    plain.address = plain.address.replacen("https", "http", 1);
    let text = line(&plain);
    assert_eq!(child(&text, &[]), "runtime_not_configured\n");
}

#[tokio::test]
async fn the_daemon_reads_the_cause_of_its_child() {
    let root = tempfile::tempdir().expect("a folder");
    let mut plain = order(root.path());
    plain.address = "https://github.com/a.exe".to_owned();
    let outcome = fetch_in_child(Path::new(DAEMON), &plain, Duration::from_secs(30)).await;
    assert_eq!(outcome, Err(Cause::RuntimeNotConfigured));
}

/// A program file below `root` with the shell `body`.
fn script(root: &Path, body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let path = root.join("child");
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("written");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("mode");
    path
}

#[tokio::test]
async fn a_stalled_child_is_ended_at_the_deadline_and_the_daemon_goes_on() {
    let root = tempfile::tempdir().expect("a folder");
    let program = script(
        root.path(),
        "echo $$ > \"$(dirname \"$0\")/pid\"; exec sleep 60",
    );
    let started = Instant::now();
    let outcome = fetch_in_child(&program, &order(root.path()), Duration::from_secs(2)).await;
    assert_eq!(outcome, Err(Cause::FetchFailed));
    assert!(started.elapsed() < Duration::from_secs(10));
    let pid = std::fs::read_to_string(root.path().join("pid")).expect("a pid");
    let gone = (0..50).any(|_| {
        std::thread::sleep(Duration::from_millis(100));
        !Command::new("kill")
            .args(["-0", pid.trim()])
            .status()
            .expect("kill")
            .success()
    });
    assert!(gone, "the stalled child still runs");
}

#[tokio::test]
async fn a_crashed_child_or_one_with_a_false_reply_is_fetch_failed() {
    let root = tempfile::tempdir().expect("a folder");
    let cases = [
        "echo done; kill -SEGV $$",
        "echo source_failed; exit 3",
        "echo listener",
        "echo '/home/x/evil.tar.gz: boom' >&2; echo 'archive /x'",
    ];
    for body in cases {
        let program = script(root.path(), body);
        let outcome = fetch_in_child(&program, &order(root.path()), Duration::from_secs(10)).await;
        assert_eq!(outcome, Err(Cause::FetchFailed), "{body}");
    }
}

#[tokio::test]
async fn a_source_error_that_the_child_reports_stays_source_failed() {
    let root = tempfile::tempdir().expect("a folder");
    let program = script(root.path(), "echo source_failed");
    let outcome = fetch_in_child(&program, &order(root.path()), Duration::from_secs(10)).await;
    assert_eq!(outcome, Err(Cause::SourceFailed));
}

#[test]
fn the_child_unpacks_only_an_archive_of_the_ordered_sha256() {
    let root = tempfile::tempdir().expect("a folder");
    let mut file = archive(&[(PROGRAM, 0o755, Some(FAKE_RUNTIME.as_bytes()))]);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).expect("read");
    let sha = hex(&Sha256::digest(&bytes));
    let mut order = order(root.path());
    std::fs::create_dir(&order.unpack_folder).expect("made");
    let checked = order.checked().expect("holds");
    assert_eq!(
        finish(archive(&[]), &sha, &order, checked),
        Err(Cause::Checksum)
    );
    assert!(!Path::new(&order.unpack_folder).join(PROGRAM).exists());
    order.sha256.clone_from(&sha);
    finish(file, &sha, &order, checked).expect("unpacked");
    assert!(Path::new(&order.unpack_folder).join(PROGRAM).is_file());
}

fn line(order: &FetchOrder) -> Vec<u8> {
    let mut text = serde_json::to_vec(order).expect("writes");
    text.push(b'\n');
    text
}

#[test]
fn the_child_refuses_a_first_host_off_the_list_before_a_connection() {
    let root = tempfile::tempdir().expect("a folder");
    let mut off = order(root.path());
    off.address = "https://127.0.0.1/llama.tar.gz".to_owned();
    assert_eq!(child(&line(&off), &[]), "runtime_not_configured\n");
    off.address = "https://evil.example/llama.tar.gz".to_owned();
    assert_eq!(child(&line(&off), &[]), "runtime_not_configured\n");
}

#[cfg(target_os = "linux")]
#[test]
fn the_child_ends_when_its_parent_is_not_the_daemon_of_the_order() {
    let root = tempfile::tempdir().expect("a folder");
    let mut stranger = order(root.path());
    stranger.parent = 1;
    assert_eq!(
        child_ending(&line(&stranger), &[]),
        (String::new(), Some(3))
    );
}

#[test]
fn the_child_ends_when_its_input_closes_before_the_whole_order() {
    let root = tempfile::tempdir().expect("a folder");
    let text = line(&order(root.path()));
    let mut child = Command::new(DAEMON)
        .arg(FETCH_MODE)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("started");
    let mut stdin = child.stdin.take().expect("stdin");
    stdin.write_all(&text[..text.len() / 2]).expect("written");
    drop(stdin);
    let mut reply = String::new();
    child
        .stdout
        .take()
        .expect("stdout")
        .read_to_string(&mut reply)
        .expect("read");
    assert!(child.wait().expect("ended").success());
    assert_eq!(reply, "runtime_not_configured\n");
}

#[test]
fn the_unpack_folder_must_be_an_empty_folder_and_not_a_link() {
    let root = tempfile::tempdir().expect("a folder");
    let file = archive(&[(PROGRAM, 0o755, Some(FAKE_RUNTIME.as_bytes()))]);
    let mut bytes = Vec::new();
    (&file).read_to_end(&mut bytes).expect("read");
    let sha = hex(&Sha256::digest(&bytes));
    let mut order = order(root.path());
    order.sha256.clone_from(&sha);
    let checked = order.checked().expect("holds");
    let into = Path::new(&order.unpack_folder).to_owned();
    std::fs::create_dir(&into).expect("made");
    std::fs::write(into.join("left"), b"x").expect("written");
    let again = || std::fs::File::open(root.path().join("a.tgz")).expect("the archive");
    std::fs::write(root.path().join("a.tgz"), &bytes).expect("written");
    assert_eq!(
        finish(again(), &sha, &order, checked),
        Err(Cause::RuntimeNotConfigured)
    );
    let target = root.path().join("elsewhere");
    std::fs::create_dir(&target).expect("made");
    std::fs::remove_dir_all(&into).expect("removed");
    std::os::unix::fs::symlink(&target, &into).expect("a link");
    assert_eq!(
        finish(again(), &sha, &order, checked),
        Err(Cause::RuntimeNotConfigured)
    );
    assert!(!target.join(PROGRAM).exists());
}

#[test]
fn a_download_hashes_the_bytes_that_it_wrote_and_stops_at_the_bound() {
    let mut file = tempfile::tempfile().expect("a file");
    let mut written = Hashed::new(&mut file, 10);
    written.write(b"hello ").expect("fits");
    written.write(b"word").expect("fits");
    assert_eq!(written.write(b"!"), Err(Cause::Size));
    let sha = written.sha256();
    let mut bytes = Vec::new();
    std::io::Seek::rewind(&mut file).expect("rewind");
    file.read_to_end(&mut bytes).expect("read");
    assert_eq!(bytes, b"hello word");
    assert_eq!(sha, hex(&Sha256::digest(&bytes)));
}
