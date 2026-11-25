//! The W2 command line against the thread runner socket protocol. No daemon or hub is needed.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::time::Duration;

use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};
use tokio::net::UnixListener;

use crate::kit::{Anima, Outcome};

const THREAD: &str = "c30b92dd-5d0d-40e2-978e-3f26d0cf9af1";
const REPO: &str = "sample-project";
const SNAPSHOT: &str = "637a30b62954ad159a5e50ce4200015e765efd07e";
const DIGEST: &str = "c8ecc4e79897b991c99191d840380b4560619bf8338ac584e9cf926a28132925c";
const REMOTE: &str = "https://example.invalid/sample/project.git";

struct Runner {
    _root: TempDir,
    listener: UnixListener,
    anima: Anima,
}

impl Runner {
    fn new() -> Self {
        // The pathname of a Unix socket must fit in sockaddr_un even on macOS.
        let root = tempfile::tempdir_in("/tmp").expect("short temporary daemon home");
        let threads = root.path().join("threads");
        fs::create_dir(&threads).expect("threads folder");
        let socket = threads.join(agent_threads::control::SOCKET);
        let listener = UnixListener::bind(&socket).expect("listen on the thread runner socket");
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).expect("private socket");
        let anima = Anima::new().with(
            "ANIMA_DAEMON_HOME",
            root.path().to_str().expect("UTF-8 daemon home"),
        );
        Self {
            _root: root,
            listener,
            anima,
        }
    }

    async fn exchange(&self, args: &[&str], result: &Value) -> (Outcome, Value) {
        let mut command = target();
        command.extend_from_slice(args);
        let mut run = None;
        let action = tokio::time::timeout(Duration::from_secs(10), async {
            let ((), action) = tokio::join!(
                async { run = Some(self.anima.run(&command).await) },
                self.reply(result)
            );
            action
        })
        .await
        .unwrap_or_else(|error| panic!("CLI exchange for {args:?}: {error}. CLI output: {run:?}"));
        (run.expect("the CLI exited"), action)
    }

    async fn reply(&self, result: &Value) -> Value {
        let (stream, _) = self.listener.accept().await.expect("CLI connects");
        let (read, mut write) = stream.into_split();
        let mut lines = BufReader::new(read).lines();
        let line = lines.next_line().await.expect("read").expect("call");
        let mut call: Value = serde_json::from_str(&line).expect("a runner call");
        assert_eq!(call["call"], "git_v2");
        assert_eq!(call["thread"], THREAD);
        assert_eq!(call["repository"], REPO);

        let fields = call.as_object_mut().expect("call object");
        let id = fields.remove("id").expect("request ID");
        for key in ["call", "thread", "repository"] {
            fields.remove(key);
        }
        let reply = json!({ "frame": "done", "id": id, "result": result });
        write
            .write_all(format!("{reply}\n").as_bytes())
            .await
            .expect("send the result");
        assert!(lines.next_line().await.expect("end of the call").is_none());
        call
    }
}

fn target() -> Vec<&'static str> {
    vec!["code", "git", "--thread", THREAD, "--repository", REPO]
}

#[tokio::test]
async fn tc7_git_reviewed_snapshot_can_be_committed_with_selected_paths() {
    let runner = Runner::new();
    let answer = json!({
        "outcome": "git", "result": "changes", "snapshot": SNAPSHOT,
        "files": [], "additions": 0, "deletions": 0, "truncated": false
    });
    let (review, action) = runner.exchange(&["changes", "--json"], &answer).await;
    assert_eq!(review.code, 0, "{review:?}");
    assert_eq!(
        action,
        json!({ "git": "changes", "scope": { "scope": "uncommitted" }, "ignore_whitespace": false })
    );
    let review = review.json();
    let reviewed = review["snapshot"].as_str().expect("reviewed snapshot");
    assert_eq!(reviewed, SNAPSHOT);

    let answer = json!({
        "outcome": "git", "result": "committed", "commit": SNAPSHOT, "branch": "review"
    });
    let args = [
        "commit",
        "--reviewed",
        reviewed,
        "--message",
        "Reviewed CLI changes",
        "--file",
        "notes with spaces.md",
        "--file",
        "src/main.rs",
        "--new-branch",
        "review",
    ];
    let (commit, action) = runner.exchange(&args, &answer).await;
    assert_eq!(commit.code, 0, "{commit:?}");
    assert!(commit.stdout.contains(" on review"), "{commit:?}");
    assert_eq!(
        action,
        json!({
            "git": "commit", "reviewed": SNAPSHOT, "message": "Reviewed CLI changes",
            "files": ["notes with spaces.md", "src/main.rs"], "new_branch": "review"
        })
    );
}

#[tokio::test]
async fn tc7_git_trust_uses_the_digest_of_the_settings_the_operator_saw() {
    let runner = Runner::new();
    let destination = format!("{REMOTE}/\u{200b}exact");
    let answer = json!({
        "outcome": "git", "result": "status", "branch": "review",
        "is_default_branch": false, "has_remote": true, "ahead": 1, "behind": 2,
        "ahead_of_default": 1, "files": [], "additions": 0, "deletions": 0,
        "push_url": destination,
        "changed": [format!("push to origin: {destination}"), "hook pre-commit\u{1b}[2J"],
        "accept": DIGEST
    });
    let (status, action) = runner.exchange(&["status"], &answer).await;
    assert_eq!(status.code, 0, "{status:?}");
    assert_eq!(action, json!({ "git": "status" }));
    assert!(
        status
            .stdout
            .contains(&format!("push to origin: {REMOTE}/\\u{{200b}}exact"))
    );
    assert!(status.stdout.contains("hook pre-commit"));
    assert!(status.stdout.contains("Ahead: 1  Behind: 2"));
    status.assert_never_shows("\u{1b}");
    status.assert_never_shows("\u{200b}");
    let shown = status
        .stdout
        .lines()
        .find_map(|line| line.strip_prefix("Acceptance digest: "))
        .expect("the digest printed with the settings");
    assert_eq!(shown, DIGEST);

    let answer = json!({
        "outcome": "git", "result": "refused",
        "why": { "refusal": "changed", "what": ["hook pre-commit"] }
    });
    let (trust, action) = runner
        .exchange(&["trust", "--shown", shown, "--json"], &answer)
        .await;
    assert_eq!(action, json!({ "git": "trust", "shown": DIGEST }));
    assert_eq!(trust.code, 6, "{trust:?}");
    assert_eq!(trust.stdout, "");
    assert_eq!(trust.error()["code"], "CONFLICT");
}

#[tokio::test]
async fn tc7_git_push_requires_a_specific_yes_for_the_default_branch() {
    let runner = Runner::new();
    let refused = json!({
        "outcome": "git", "result": "refused", "why": { "refusal": "default_branch" }
    });
    let (push, action) = runner.exchange(&["push", "--yes"], &refused).await;
    assert_eq!(
        action,
        json!({ "git": "push", "default_branch_allowed": false })
    );
    assert_eq!(push.code, 5, "{push:?}");
    assert!(push.stderr.contains("--allow-default-branch"));
    let (pr, action) = runner.exchange(&["pr"], &refused).await;
    assert_eq!(action, json!({ "git": "pull_request" }));
    assert_eq!(pr.code, 5, "{pr:?}");
    assert!(pr.stderr.contains("feature branch"), "{pr:?}");
    pr.assert_never_shows("--allow-default-branch");

    let pushed = json!({
        "outcome": "git", "result": "pushed", "remote": "origin",
        "url": REMOTE, "branch": "main", "up_to_date": false
    });
    let (push, action) = runner
        .exchange(&["push", "--allow-default-branch"], &pushed)
        .await;
    assert_eq!(
        action,
        json!({ "git": "push", "default_branch_allowed": true })
    );
    assert_eq!(push.code, 0, "{push:?}");
    assert!(push.stdout.contains(REMOTE));
}

#[tokio::test]
async fn tc7_git_review_shows_hidden_characters_and_literal_escape_notation_distinctly() {
    let runner = Runner::new();
    let patch = concat!("@@ -1 +1 @@\n-value\\u{200b}\n", "+value\u{200b}\u{1b}\t\n");
    let answer = json!({
        "outcome": "git", "result": "changes", "snapshot": SNAPSHOT, "additions": 1, "deletions": 1,
        "files": [{"path": "notes.txt", "kind": "modified", "additions": 1, "deletions": 1, "patch": patch}],
        "truncated": false,
    });
    let (review, _) = runner.exchange(&["changes"], &answer).await;
    assert_eq!(review.code, 0, "{review:?}");
    assert!(review.stdout.contains("-value\\\\u{200b}\n"), "{review:?}");
    assert!(
        review.stdout.contains("+value\\u{200b}\\u{1b}\\u{9}\n"),
        "{review:?}"
    );
    for hidden in ["\u{200b}", "\u{1b}"] {
        review.assert_never_shows(hidden);
    }
}

#[tokio::test]
async fn tc7_git_remove_reports_both_the_snapshot_and_the_recovery_directory() {
    let runner = Runner::new();
    let recovery = "/fixture/worktrees/.anima-retired/sample/worktree";
    let kept = "refs/anima/kept/c30b92dd-5d0d-40e2-978e-3f26d0cf9af1";
    let answer =
        json!({ "outcome": "git", "result": "removed", "kept": kept, "recovery": recovery });
    let (removed, action) = runner.exchange(&["remove", "--yes"], &answer).await;
    assert_eq!(action, json!({ "git": "remove" }));
    assert_eq!(removed.code, 0, "{removed:?}");
    assert!(
        removed.stdout.contains(&format!("Git snapshot: {kept}.")),
        "{removed:?}"
    );
    assert!(
        removed
            .stdout
            .contains(&format!("Complete directory: {recovery}")),
        "{removed:?}"
    );
}

#[tokio::test]
async fn tc7_git_remove_reports_the_saved_ref_when_recovery_has_no_directory() {
    let runner = Runner::new();
    let original = "/fixture/worktrees/sample";
    let kept = "refs/anima/kept/c30b92dd-5d0d-40e2-978e-3f26d0cf9af1";
    let answer =
        json!({ "outcome": "git", "result": "removed", "kept": kept, "original": original });
    let (removed, _) = runner.exchange(&["remove", "--yes"], &answer).await;
    assert_eq!(removed.code, 0, "{removed:?}");
    assert!(
        removed
            .stdout
            .contains(&format!("Former path: {original}.")),
        "{removed:?}"
    );
    assert!(
        removed
            .stdout
            .contains("No recovery directory was created."),
        "{removed:?}"
    );
    assert!(
        removed.stdout.contains(&format!("Git snapshot: {kept}.")),
        "{removed:?}"
    );
}
