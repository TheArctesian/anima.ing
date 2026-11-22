//! The hooks of the repository by the access of the thread (ruling of the operator): a
//! confined thread reaches no hook by any action or setting; a thread with full access runs
//! the hooks of a commit and a push, started as the thread and never by the runner's `git`.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use agent_threads::git::{GitAction, GitOutcome, GitRefusal};

use super::trusted::{HOOKS, PUSH, UNCOMMITTED, commit};
use super::{Bench, PERSON, refused, target};
use crate::threads::git::place::Home;
use crate::threads::git::run::hooks::{Access, AsThread, Ended};
use crate::threads::git::{Caller, Engine};

const FULL: Caller = Caller {
    access: Access::Full,
    ..PERSON
};

impl Bench {
    /// The engine of the bench with a way to start hooks: here the `git` of the test itself,
    /// in place of the launcher of a thread. `started` counts each start.
    fn with_hooks(&self, started: &Arc<AtomicUsize>) -> Engine {
        let records = self.home.with_file_name("records");
        let home = Home::at(&self.home, &records).expect("a home");
        let started = Arc::clone(started);
        let as_thread = AsThread::new(move |run| {
            started.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                let done = tokio::process::Command::new("git")
                    .args(&run.args)
                    .current_dir(&run.dir)
                    .envs(run.env.iter().map(|(name, value)| (name, value)))
                    .output()
                    .await?;
                Ok(Ended {
                    code: done.status.code(),
                    said: String::from_utf8_lossy(&done.stderr).into_owned(),
                })
            })
        });
        Engine::new(self.git.clone(), home).with_hooks(as_thread)
    }

    /// Writes the hook `name` of the project with `body`.
    fn one_hook(&self, name: &str, body: &str) {
        let file = self.project.join(".git/hooks").join(name);
        std::fs::create_dir_all(file.parent().expect("the hooks")).expect("the hooks");
        std::fs::write(&file, format!("#!/bin/sh\n{body}\n")).expect("a hook");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let runs = std::fs::Permissions::from_mode(0o755);
            std::fs::set_permissions(&file, runs).expect("a mode");
        }
    }
}

/// Each way the repository has to name a hook, each of which would make the file `ran`: the
/// folder of the hooks, a second folder by `core.hooksPath`, the hooks of the configuration,
/// and a file that the configuration takes in for the branches of threads.
async fn each_hook_planted(bench: &Bench, ran: &Path) {
    let touch = format!("touch \"{}\"", ran.display());
    bench.hook(&touch);
    let root = bench.home.parent().expect("the root");
    let second = root.join("second-hooks");
    std::fs::create_dir_all(&second).expect("a folder");
    let mut taken_in = String::new();
    for event in HOOKS {
        let from = bench.project.join(".git/hooks").join(event);
        std::fs::copy(from, second.join(event)).expect("a hook");
        let _ = writeln!(
            taken_in,
            "[hook \"in-{event}\"]\n\tevent = {event}\n\tcommand = {touch}"
        );
        let key = format!("hook.set-{event}.event");
        bench.git_in(&bench.project, &["config", &key, event]).await;
        let key = format!("hook.set-{event}.command");
        bench
            .git_in(&bench.project, &["config", &key, &touch])
            .await;
    }
    let file = root.join("planted.inc");
    std::fs::write(&file, taken_in).expect("the file");
    let file = file.to_string_lossy().into_owned();
    let when = "includeIf.onbranch:anima/**.path";
    bench.git_in(&bench.project, &["config", when, &file]).await;
    let path = second.to_string_lossy().into_owned();
    let hooks_path = ["config", "core.hooksPath", path.as_str()];
    bench.git_in(&bench.project, &hooks_path).await;
    let _ = std::fs::remove_file(ran);
}

#[tokio::test]
async fn a_confined_thread_reaches_no_hook_by_any_action_or_setting() {
    let bench = Bench::new().await;
    bench.with_origin().await;
    let ran = bench.home.with_file_name("RAN");
    each_hook_planted(&bench, &ran).await;
    let started = Arc::new(AtomicUsize::new(0));
    let engine = bench.with_hooks(&started);
    let call = |action| engine.call(PERSON, target(), action);

    // The worktree is made with each hook in place, by the engine that could start hooks.
    let plan = super::worktree::Plan {
        thread: super::THREAD,
        project: &bench.project,
        base: None,
    };
    let folder = engine.worktree(plan).await.expect("a worktree");
    let folder = folder.worktree.folder().to_owned();
    let GitOutcome::Status(status) = call(GitAction::Status).await else {
        panic!("no status");
    };
    let trust = GitAction::Trust {
        shown: status.accept,
    };
    assert_eq!(call(trust).await, GitOutcome::Trusted);
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    engine.turn_ended(target(), 1).await.expect("a checkpoint");
    let GitOutcome::Changes(shown) = call(UNCOMMITTED).await else {
        panic!("no changes");
    };
    let reviewed = shown.snapshot.expect("a snapshot");
    let committed = call(commit(&reviewed, None)).await;
    assert!(
        matches!(committed, GitOutcome::Committed { .. }),
        "{committed:?}"
    );
    std::fs::write(folder.join("b.txt"), "b\n").expect("a file");
    let GitOutcome::Changes(shown) = call(UNCOMMITTED).await else {
        panic!("no changes");
    };
    let on_new = GitAction::Commit {
        message: "on a new branch".to_owned(),
        reviewed: shown.snapshot.expect("a snapshot"),
        files: None,
        new_branch: Some("feature/new".to_owned()),
    };
    let committed = call(on_new).await;
    assert!(
        matches!(committed, GitOutcome::Committed { .. }),
        "{committed:?}"
    );
    let pushed = call(PUSH).await;
    assert!(matches!(pushed, GitOutcome::Pushed { .. }), "{pushed:?}");
    let reverted = call(GitAction::Revert { turn: 0 }).await;
    assert!(
        matches!(reverted, GitOutcome::Reverted { .. }),
        "{reverted:?}"
    );
    let removed = call(GitAction::Remove).await;
    assert!(matches!(removed, GitOutcome::Removed { .. }), "{removed:?}");

    assert_eq!(started.load(Ordering::SeqCst), 0, "a hook was started");
    assert!(!ran.exists(), "a hook of the repository ran");
}

#[tokio::test]
async fn a_thread_with_full_access_runs_the_hooks_of_a_commit_and_a_push_as_the_thread() {
    let bench = Bench::new().await;
    bench.with_origin().await;
    let folder = bench.worktree().await;
    let root = bench.home.parent().expect("the root");
    let seen = root.join("SEEN");
    let started = Arc::new(AtomicUsize::new(0));
    let engine = bench.with_hooks(&started);
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    std::fs::write(folder.join("new.txt"), "new\n").expect("a file");
    let reviewed = bench.reviewed().await;

    // The hook sees the files of the commit as staged, and its refusal stops the commit.
    let refuse = format!(
        "git diff --cached --name-only > \"{}\"; echo no >&2; exit 1",
        seen.display()
    );
    bench.one_hook("pre-commit", &refuse);
    let head = bench.git_in(&folder, &["rev-parse", "HEAD"]).await;
    let stopped = engine
        .call(FULL, target(), commit(&reviewed, Some(&["new.txt"])))
        .await;
    assert!(
        matches!(refused(&stopped), Some(GitRefusal::Failed { said }) if said.contains("pre-commit")),
        "{stopped:?}"
    );
    assert_eq!(
        std::fs::read_to_string(&seen).expect("the hook ran"),
        "new.txt\n"
    );
    assert_eq!(bench.git_in(&folder, &["rev-parse", "HEAD"]).await, head);

    bench.one_hook("pre-commit", "exit 0");
    bench.one_hook("commit-msg", "echo '[checked]' >> \"$1\"");
    let after = root.join("AFTER");
    bench.one_hook("post-commit", &format!("touch \"{}\"", after.display()));
    let lines = root.join("PUSHED");
    bench.one_hook("pre-push", &format!("cat > \"{}\"", lines.display()));
    // A hook that the runner's own `git` would run for a moved ref still does not run.
    let ran = root.join("RAN");
    let moved = format!("touch \"{}\"", ran.display());
    bench.one_hook("reference-transaction", &moved);

    let committed = engine.call(FULL, target(), commit(&reviewed, None)).await;
    let GitOutcome::Committed { commit: made, .. } = committed else {
        panic!("no commit: {committed:?}");
    };
    let message = ["log", "-1", "--format=%B"];
    assert_eq!(
        bench.git_in(&folder, &message).await,
        "by the person\n[checked]"
    );
    assert!(after.exists(), "the hook after the commit did not run");
    let pushed = engine.call(FULL, target(), PUSH).await;
    assert!(matches!(pushed, GitOutcome::Pushed { .. }), "{pushed:?}");
    let line = std::fs::read_to_string(&lines).expect("the hook of the push ran");
    assert!(
        line.starts_with(&format!("refs/heads/anima/12345678 {made} ")),
        "{line}"
    );
    assert!(
        started.load(Ordering::SeqCst) >= 5,
        "the hooks were not started as the thread"
    );
    assert!(!ran.exists(), "the runner's own git ran a hook");
}

#[tokio::test]
async fn full_access_runs_no_hook_when_the_runner_gave_no_way_to_start_one() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let ran = folder.with_file_name("RAN");
    bench.hook(&format!("touch \"{}\"; exit 1", ran.display()));
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    let committed = bench.call(FULL, commit("", None)).await;
    assert!(
        matches!(committed, GitOutcome::Committed { .. }),
        "{committed:?}"
    );
    assert!(!ran.exists(), "a hook of the repository ran");
}
