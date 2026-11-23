use std::ffi::CString;
use std::io::{Read as _, Write as _};
use std::os::fd::OwnedFd;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::{Command, Stdio};

use anima_daemon::model::folders::private_below;
use anima_daemon::model::pull::child::{PULL_MODE, address};
use anima_daemon::model::pull::partial::{open_partial, remove_below, size_and_sha256};
use anima_daemon::model::pull::{Cache, delete, pull};
use daemon_core::model_work::cause::Cause;
use daemon_core::model_work::machine::MachineSettings;
use daemon_core::model_work::names::Format;
use daemon_core::model_work::order::{Parameters, PullFile, PullSource};
use daemon_core::model_work::pull::PullOrder;
use serde_json::json;
use uuid::Uuid;

const DAEMON: &str = env!("CARGO_BIN_EXE_anima-daemon");

fn source() -> PullSource {
    PullSource {
        organisation: "unsloth".to_owned(),
        repository: "Qwen3-8B-GGUF".to_owned(),
        revision: "a".repeat(40),
    }
}

fn files() -> Vec<PullFile> {
    vec![PullFile {
        name: "Qwen3-8B-Q4_K_M.gguf".to_owned(),
        size_bytes: 1 << 20,
        sha256: "b".repeat(64),
    }]
}

fn parameters(copy: Uuid, gated: bool) -> Parameters {
    Parameters::Pull {
        copy,
        variant: Uuid::nil(),
        source: source(),
        format: Format::Gguf,
        files: files(),
        gated,
    }
}

fn folder(root: &Path, relative: &str) -> OwnedFd {
    private_below(root, Path::new(relative)).expect("folder")
}

/// The pull process with `order` on its input and `variables` as its environment.
fn child(order: &[u8], variables: &[(&str, &str)]) -> String {
    let mut child = Command::new(DAEMON)
        .arg(PULL_MODE)
        .env_clear()
        .envs(variables.iter().copied())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("started");
    let mut input = child.stdin.take().expect("stdin");
    input.write_all(order).expect("order");
    let mut reply = String::new();
    child
        .stdout
        .take()
        .expect("stdout")
        .read_to_string(&mut reply)
        .expect("read");
    assert_eq!(child.wait().expect("ended").code(), Some(0));
    drop(input);
    reply
}

fn order_line(models: &Path, change: impl FnOnce(&mut PullOrder)) -> Vec<u8> {
    let mut order = PullOrder {
        copy: Uuid::now_v7(),
        source: source(),
        format: "gguf".to_owned(),
        files: files(),
        models: models.to_string_lossy().into_owned(),
        reserve: 0,
        parent: std::process::id(),
    };
    change(&mut order);
    let mut line = serde_json::to_vec(&order).expect("order");
    line.push(b'\n');
    line
}

#[test]
fn pull_004_the_pull_process_refuses_a_member_that_does_not_hold() {
    let root = tempfile::tempdir().expect("root");
    let line = order_line(root.path(), |order| {
        order.files[0].name = "../x.gguf".to_owned();
    });

    assert_eq!(child(&line, &[]), "member_unknown\n");
    assert!(!root.path().join("partial").exists(), "nothing is written");
}

#[test]
fn sec_c_003_the_pull_process_refuses_an_environment() {
    let root = tempfile::tempdir().expect("root");
    let line = order_line(root.path(), |_| {});

    assert_eq!(child(&line, &[("HF_TOKEN", "x")]), "fetch_failed\n");
}

#[test]
fn sf_381_the_address_keeps_the_slash_of_a_folder() {
    let url = address(&source(), "q4/Qwen3 8B.gguf").expect("address");

    assert_eq!(
        url.as_str(),
        format!(
            "https://huggingface.co/unsloth/Qwen3-8B-GGUF/resolve/{}/q4/Qwen3%208B.gguf",
            "a".repeat(40)
        )
    );
}

#[test]
fn r_408_a_partial_file_is_never_opened_through_a_link() {
    let root = tempfile::tempdir().expect("root");
    let copy = folder(root.path(), "partial/c");
    let outside = root.path().join("outside");
    std::fs::write(&outside, b"keep").expect("outside");
    symlink(&outside, root.path().join("partial/c/a.gguf")).expect("link");
    std::fs::write(root.path().join("partial/c/b.gguf"), b"b").expect("file");
    std::fs::hard_link(
        root.path().join("partial/c/b.gguf"),
        root.path().join("second"),
    )
    .expect("hard link");

    assert!(open_partial(&copy, "a.gguf").is_err());
    assert!(
        open_partial(&copy, "b.gguf").is_err(),
        "a file with a second name"
    );
    assert_eq!(std::fs::read(&outside).expect("outside"), b"keep");
}

#[test]
fn pull_006_a_partial_file_is_opened_again_with_its_bytes() {
    let root = tempfile::tempdir().expect("root");
    let copy = folder(root.path(), "partial/c");
    open_partial(&copy, "a.gguf")
        .expect("new")
        .write_all(b"abc")
        .expect("write");

    let again = open_partial(&copy, "a.gguf").expect("again");

    assert_eq!(again.metadata().expect("meta").len(), 3);
    let (size, sha) = size_and_sha256(&copy, "a.gguf").expect("read");
    assert_eq!(size, 3);
    assert_eq!(
        sha,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn pull_009_a_removal_removes_a_link_and_never_what_it_names() {
    let root = tempfile::tempdir().expect("root");
    let copy = Uuid::now_v7();
    let kept = root.path().join("kept");
    std::fs::create_dir(&kept).expect("kept");
    std::fs::write(kept.join("file"), b"keep").expect("file");
    folder(root.path(), "copies");
    symlink(&kept, root.path().join("copies").join(copy.to_string())).expect("link");
    folder(root.path(), &format!("partial/{copy}"));
    std::fs::write(root.path().join(format!("partial/{copy}/a.gguf")), b"a").expect("part");

    assert_eq!(delete(root.path(), copy), Ok(None));

    assert!(kept.join("file").exists());
    assert!(!root.path().join("copies").join(copy.to_string()).exists());
    assert!(!root.path().join("partial").join(copy.to_string()).exists());
    assert_eq!(delete(root.path(), copy), Ok(None), "a copy that is gone");
}

#[test]
fn remove_below_refuses_folders_deeper_than_a_copy() {
    let root = tempfile::tempdir().expect("root");
    let top = folder(root.path(), "");
    std::fs::create_dir_all(root.path().join("d/1/2/3/4/5")).expect("deep");

    assert!(remove_below(&top, &CString::new("d").expect("name"), 0).is_err());
}

fn cache<'a>(models: &'a Path, settings: &'a MachineSettings) -> Cache<'a> {
    Cache {
        models,
        reserve: 0,
        program: Path::new(DAEMON),
        settings,
    }
}

#[tokio::test]
async fn pull_010_a_gated_pull_and_the_unbuilt_checks_refuse_before_any_file() {
    let root = tempfile::tempdir().expect("root");
    let settings = MachineSettings::default();
    let second = MachineSettings {
        second_hash: true,
        ..MachineSettings::default()
    };
    let copy = Uuid::now_v7();

    let gated = pull(&parameters(copy, true), &cache(root.path(), &settings)).await;
    let hashed = pull(&parameters(copy, false), &cache(root.path(), &second)).await;

    assert_eq!(gated, Err(Cause::SecretUnknown));
    assert_eq!(hashed, Err(Cause::HashMismatch));
    assert!(!root.path().join("partial").exists());
}

#[tokio::test]
async fn pull_005_a_copy_that_was_moved_already_is_downloaded() {
    let root = tempfile::tempdir().expect("root");
    let settings = MachineSettings::default();
    let copy = Uuid::now_v7();
    folder(root.path(), &format!("copies/{copy}"));

    let pulled = pull(&parameters(copy, false), &cache(root.path(), &settings)).await;

    let model = json!({ "copy": copy, "copyState": "downloaded", "bytes": 1 << 20, "files": 1 });
    assert_eq!(pulled, Ok(Some(model)));
}

#[tokio::test]
async fn pull_008_a_fifth_partial_copy_is_refused() {
    let root = tempfile::tempdir().expect("root");
    let settings = MachineSettings::default();
    for _ in 0..4 {
        folder(root.path(), &format!("partial/{}", Uuid::now_v7()));
    }

    let pulled = pull(
        &parameters(Uuid::now_v7(), false),
        &cache(root.path(), &settings),
    )
    .await;

    assert_eq!(pulled, Err(Cause::DiskShare));
}

#[tokio::test]
async fn pull_008_a_pull_that_does_not_fit_above_the_reserve_is_refused() {
    let root = tempfile::tempdir().expect("root");
    let settings = MachineSettings::default();
    let mut place = cache(root.path(), &settings);
    place.reserve = u64::MAX / 2;

    let pulled = pull(&parameters(Uuid::now_v7(), false), &place).await;

    assert_eq!(pulled, Err(Cause::DiskShare));
}
