//! The launcher program as the daemon runs it. The release key is built into it, so no list that a
//! test signs can pass: these tests prove the refusals; the start of a listed runtime is part of
//! the human check of task L1-6c.

#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const LAUNCHER: &str = env!("CARGO_BIN_EXE_anima-model-launcher");

/// The files of one start: a stand-in `cgroup.procs`, the failure file, a list, its signature and
/// a program.
struct Start {
    folder: tempfile::TempDir,
}

impl Start {
    fn new(list: &[u8], signature: &str) -> Self {
        let folder = tempfile::tempdir().expect("a folder");
        std::fs::write(folder.path().join("cgroup.procs"), "").expect("procs");
        std::fs::write(folder.path().join("runtime-list.json"), list).expect("list");
        std::fs::write(folder.path().join("runtime-list.json.sig"), signature).expect("sig");
        std::fs::copy("/bin/true", folder.path().join("program")).expect("program");
        Self { folder }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.folder.path().join(name)
    }

    fn run(&self) -> Output {
        Command::new(LAUNCHER)
            .arg(self.path("runtime-list.json"))
            .arg(self.path("runtime-list.json.sig"))
            .arg(self.path("cgroup.procs"))
            .arg(std::process::id().to_string())
            .arg(self.path("failure"))
            .arg(self.path("program"))
            .output()
            .expect("the launcher ran")
    }

    fn failure(&self) -> String {
        std::fs::read_to_string(self.path("failure")).unwrap_or_default()
    }
}

fn list_of(program: &Path) -> Vec<u8> {
    use sha2::{Digest as _, Sha256};
    let bytes = std::fs::read(program).expect("program");
    let sha256 = model_launcher::list::hex(&Sha256::digest(&bytes));
    serde_json::to_vec(&serde_json::json!({
        "sequence": 1,
        "runtimes": [{
            "runtime": "llama_server", "version": "b1", "backend": "cpu", "os": "linux",
            "architecture": std::env::consts::ARCH, "program": "program", "sha256": sha256,
        }],
    }))
    .expect("json")
}

#[test]
fn the_launcher_names_its_protocol() {
    let output = Command::new(LAUNCHER)
        .arg("--protocol")
        .output()
        .expect("ran");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        model_launcher::protocol_line()
    );
}

#[test]
fn a_program_on_a_list_that_the_release_key_did_not_sign_is_refused_as_not_signed() {
    let start = Start::new(b"", "");
    let list = list_of(&start.path("program"));
    std::fs::write(start.path("runtime-list.json"), list).expect("list");
    let output = start.run();
    assert_eq!(output.status.code(), Some(125));
    assert_eq!(start.failure(), "125");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
}

#[test]
fn a_missing_list_is_refused_as_not_signed() {
    let start = Start::new(b"", "");
    std::fs::remove_file(start.path("runtime-list.json")).expect("removed");
    assert_eq!(start.run().status.code(), Some(125));
    assert_eq!(start.failure(), "125");
}

#[test]
fn a_program_that_is_a_link_is_refused_before_the_list_is_read() {
    let start = Start::new(b"", "");
    std::fs::remove_file(start.path("program")).expect("removed");
    std::os::unix::fs::symlink("/bin/true", start.path("program")).expect("link");
    assert_eq!(start.run().status.code(), Some(124));
    assert_eq!(start.failure(), "124");
}

#[test]
fn a_launcher_with_too_few_arguments_runs_nothing() {
    let output = Command::new(LAUNCHER)
        .args(["list", "sig", "procs"])
        .output()
        .expect("ran");
    assert_eq!(output.status.code(), Some(124));
}
