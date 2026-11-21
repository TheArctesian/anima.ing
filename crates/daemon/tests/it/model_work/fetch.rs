//! The child fetch process (task L1-6b): the child in its mode, and the daemon side with a
//! stalled and a crashed child.

use std::io::{Read as _, Write as _};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anima_daemon::model::fetch::child::{FETCH_MODE, finish};
use anima_daemon::model::fetch::hex;
use anima_daemon::model::fetch::spawn::fetch_in_child;
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
    }
}

/// Runs the child with `input` and the environment `variables`, and gives its reply.
fn child(input: &[u8], variables: &[(&str, &str)]) -> String {
    let mut child = Command::new(DAEMON)
        .arg(FETCH_MODE)
        .env_clear()
        .envs(variables.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("started");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input)
        .expect("written");
    let mut reply = String::new();
    child
        .stdout
        .take()
        .expect("stdout")
        .read_to_string(&mut reply)
        .expect("read");
    assert!(child.wait().expect("ended").success());
    reply
}

#[test]
fn the_child_refuses_to_work_with_any_environment_variable() {
    let root = tempfile::tempdir().expect("a folder");
    let text = serde_json::to_vec(&order(root.path())).expect("writes");
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
    let text = serde_json::to_vec(&plain).expect("writes");
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
    assert_eq!(outcome, Err(Cause::SourceFailed));
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
async fn a_crashed_child_or_one_with_a_false_reply_is_source_failed() {
    let root = tempfile::tempdir().expect("a folder");
    let cases = [
        "echo done; kill -SEGV $$",
        "echo done; exit 3",
        "echo listener",
    ];
    for body in cases {
        let program = script(root.path(), body);
        let outcome = fetch_in_child(&program, &order(root.path()), Duration::from_secs(10)).await;
        assert_eq!(outcome, Err(Cause::SourceFailed), "{body}");
    }
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
