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
use crate::threads::git::run::hooks::Access;
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
        Engine::new(self.git.clone(), home)
            .with_launcher(super::copy::as_the_person(Some(Arc::clone(started))))
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
    // A hook is a file that each thread of the repository can write: none runs before the
    // person accepted it as it is.
    let unaccepted = engine
        .call(FULL, target(), commit(&reviewed, Some(&["new.txt"])))
        .await;
    assert!(
        matches!(refused(&unaccepted), Some(GitRefusal::Changed { what }) if what == &["hook pre-commit"]),
        "{unaccepted:?}"
    );
    assert_eq!(started.load(Ordering::SeqCst), 0, "a hook ran before a yes");
    accept(&engine).await;
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
}

#[tokio::test]
async fn the_hooks_of_a_commit_and_a_push_run_in_their_order_and_no_other_hook_does() {
    let bench = Bench::new().await;
    bench.with_origin().await;
    let folder = bench.worktree().await;
    let root = bench.home.parent().expect("the root");
    let started = Arc::new(AtomicUsize::new(0));
    let engine = bench.with_hooks(&started);
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    let reviewed = bench.reviewed().await;
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
    accept(&engine).await;

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
        started.load(Ordering::SeqCst) >= 4,
        "the hooks were not started as the thread"
    );
    assert!(!ran.exists(), "the runner's own git ran a hook");
}

/// The yes of the person, in a thread with full access, to the hooks as they are now.
async fn accept(engine: &Engine) {
    let GitOutcome::Status(status) = engine.call(FULL, target(), GitAction::Status).await else {
        panic!("no status");
    };
    let trust = GitAction::Trust {
        shown: status.accept,
    };
    assert_eq!(
        engine.call(FULL, target(), trust).await,
        GitOutcome::Trusted
    );
}

#[tokio::test]
async fn a_hook_that_a_different_thread_changed_runs_only_after_a_new_yes() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let root = bench.home.parent().expect("the root");
    let ran = root.join("RAN");
    let started = Arc::new(AtomicUsize::new(0));
    let engine = bench.with_hooks(&started);
    bench.one_hook("pre-commit", "exit 0");
    accept(&engine).await;
    // What a confined thread of the same repository can do: write the shared hook, or name a
    // folder of hooks of its own in the shared configuration.
    let planted = format!("touch \"{}\"", ran.display());
    bench.one_hook("pre-commit", &planted);
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    let reviewed = bench.reviewed().await;
    let stopped = engine.call(FULL, target(), commit(&reviewed, None)).await;
    assert!(
        matches!(refused(&stopped), Some(GitRefusal::Changed { .. })),
        "{stopped:?}"
    );
    bench.one_hook("pre-commit", "exit 0");
    let elsewhere = root.join("planted-hooks");
    std::fs::create_dir_all(&elsewhere).expect("a folder");
    std::fs::copy(
        bench.project.join(".git/hooks/pre-commit"),
        elsewhere.join("pre-commit"),
    )
    .expect("a hook");
    std::fs::write(
        elsewhere.join("pre-commit"),
        format!("#!/bin/sh\n{planted}\n"),
    )
    .expect("a hook");
    let path = elsewhere.to_string_lossy().into_owned();
    let set = ["config", "core.hooksPath", path.as_str()];
    bench.git_in(&bench.project, &set).await;
    let stopped = engine.call(FULL, target(), commit(&reviewed, None)).await;
    assert!(
        matches!(refused(&stopped), Some(GitRefusal::Changed { .. })),
        "{stopped:?}"
    );
    assert_eq!(started.load(Ordering::SeqCst), 0, "a hook was started");
    assert!(
        !ran.exists(),
        "a planted hook ran in the thread with full access"
    );
    // A confined thread is asked nothing about hooks, and its yes accepts none.
    assert_eq!(bench.status(PERSON).await.changed, Vec::<String>::new());
}

#[tokio::test]
async fn a_hook_changed_after_its_check_does_not_run_its_new_text() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let ran = bench.home.parent().expect("the root").join("RAN");
    let hook = bench.project.join(".git/hooks/pre-commit");
    let planted = format!("#!/bin/sh\ntouch \"{}\"\n", ran.display());
    // A thread that writes the hook in the moment between its check and its start.
    let person = super::copy::as_the_person(None);
    let swapping = crate::threads::git::run::hooks::AsThread::new(move |run| {
        if run.program.file_name().is_some_and(|name| name != "git") {
            std::fs::write(&hook, &planted).expect("the planted hook");
        }
        let person = person.clone();
        Box::pin(async move { person.start(run).await })
    });
    let records = bench.home.with_file_name("records");
    let home = Home::at(&bench.home, &records).expect("a home");
    let engine = Engine::new(bench.git.clone(), home).with_launcher(swapping);
    bench.one_hook("pre-commit", "exit 0");
    accept(&engine).await;
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    let reviewed = bench.reviewed().await;
    let _ = engine.call(FULL, target(), commit(&reviewed, None)).await;
    assert!(!ran.exists(), "the text written after the check ran");
}
