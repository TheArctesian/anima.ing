//! The checked copy of the history of a project (ruling of the operator): the runner reads
//! the object store of the person only through the launcher of the thread, keeps one copy for
//! each project, and no `git` of the runner reads the store of the person after that.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_threads::git::{GitAction, GitOutcome, GitRefusal, Preparing};
use uuid::Uuid;

use super::trusted::UNCOMMITTED;
use super::{Bench, PERSON, THREAD, refused, target};
use crate::threads::git::place::Home;
use crate::threads::git::run::hooks::{AsThread, Ended};
use crate::threads::git::{Engine, Target, worktree};

/// The launcher of the tests: it starts the program as the person of the test would, in
/// place of the launcher of a thread. `hooks` counts each start of a program that is not git.
pub(super) fn as_the_person(hooks: Option<Arc<AtomicUsize>>) -> AsThread {
    AsThread::new(move |run| {
        let is_git = run.program.file_name().is_some_and(|name| name == "git");
        if let (false, Some(hooks)) = (is_git, &hooks) {
            hooks.fetch_add(1, Ordering::SeqCst);
        }
        Box::pin(async move {
            let input = match &run.input {
                Some(file) => std::fs::File::open(file)?.into(),
                None => std::process::Stdio::null(),
            };
            let done = tokio::process::Command::new(&run.program)
                .args(&run.args)
                .current_dir(&run.dir)
                .envs(run.env.iter().map(|(name, value)| (name, value)))
                .stdin(input)
                .output();
            let done = tokio::time::timeout(run.within, done).await??;
            Ok(Ended {
                code: done.status.code(),
                said: String::from_utf8_lossy(&done.stderr).into_owned(),
            })
        })
    })
}

impl Bench {
    /// An engine on the records of the bench with the launcher `as_thread`, or with none.
    fn engine_with(&self, as_thread: Option<AsThread>) -> Engine {
        let records = self.home.with_file_name("records");
        let home = Home::at(&self.home, &records).expect("a home");
        let engine = Engine::new(self.git.clone(), home);
        match as_thread {
            Some(as_thread) => engine.with_launcher(as_thread),
            None => engine,
        }
    }

    fn plan(&self, thread: Uuid) -> worktree::Plan<'_> {
        worktree::Plan {
            thread,
            project: &self.project,
            base: None,
        }
    }
}

#[tokio::test]
async fn no_git_of_the_runner_reads_the_object_store_of_the_person() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    // The store of the person is gone: what the runner needs is in its own copy.
    let objects = bench.project.join(".git/objects");
    std::fs::rename(&objects, objects.with_file_name("objects-away")).expect("the move");
    let GitOutcome::Status(status) = bench.call(PERSON, GitAction::Status).await else {
        panic!("no status");
    };
    assert_eq!(status.files.len(), 1);
    let GitOutcome::Changes(shown) = bench.call(PERSON, UNCOMMITTED).await else {
        panic!("no changes");
    };
    let patch = shown.files[0].patch.as_deref().unwrap_or_default();
    assert!(patch.contains("-one\n+changed"), "{patch}");
    bench.turn_ended(1).await;
    let reverted = bench.call(PERSON, GitAction::Revert { turn: 0 }).await;
    assert!(
        matches!(reverted, GitOutcome::Reverted { .. }),
        "{reverted:?}"
    );
}

#[tokio::test]
async fn a_worktree_is_refused_when_the_runner_gave_no_way_to_read_as_the_thread() {
    let bench = Bench::new().await;
    let made = bench.engine_with(None).worktree(bench.plan(THREAD)).await;
    assert!(
        matches!(&made, Err(GitRefusal::Failed { said }) if said.contains("rights of the thread")),
        "{made:?}"
    );
    // Nothing of the try is left: the next one starts clean.
    assert!(!bench.home.join(THREAD.to_string()).join("engine").exists());
    bench.worktree().await;
}

#[tokio::test]
async fn two_threads_of_one_project_get_their_worktrees_at_the_same_time_from_one_copy() {
    let bench = Bench::new().await;
    // Two IDs with different first digits: the branch of a thread has its first eight.
    let (first, second) = (Uuid::from_u128(1 << 100), Uuid::from_u128(2 << 100));
    let (one, two) = tokio::join!(
        bench.engine.worktree(bench.plan(first)),
        bench.engine.worktree(bench.plan(second)),
    );
    let (one, two) = (one.expect("a worktree"), two.expect("a worktree"));
    assert_ne!(one.worktree.folder(), two.worktree.folder());
    assert_eq!(
        one.worktree.record().shared,
        two.worktree.record().shared,
        "one copy for the project"
    );
    for thread in [first, second] {
        let of = Target {
            thread,
            repository: "engine",
        };
        let status = bench.engine.call(PERSON, of, GitAction::Status).await;
        assert!(matches!(status, GitOutcome::Status(_)), "{status:?}");
    }
    let used = bench.engine.disk();
    assert_eq!(used.len(), 1);
    assert_eq!(
        (used[0].project.as_path(), used[0].worktrees),
        (bench.project.as_path(), 2)
    );
    assert!(used[0].bytes > 0);
}

#[tokio::test]
async fn a_cancelled_worktree_leaves_nothing_and_the_steps_are_shown() {
    let bench = Bench::new().await;
    // A launcher that never ends, as the copy of a very large history.
    let never = AsThread::new(|_| Box::pin(std::future::pending()));
    let slow = bench.engine_with(Some(never));
    let steps = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&steps);
    let shown = move |step: Preparing| seen.lock().expect("the steps").push(step);
    let making = slow.worktree_shown(bench.plan(THREAD), Some(&shown));
    // The person cancels: the runner drops the call.
    let cancelled = tokio::time::timeout(Duration::from_secs(4), making).await;
    assert!(cancelled.is_err(), "the copy ended by itself");
    assert_eq!(*steps.lock().expect("the steps"), [Preparing::History]);
    assert!(!bench.home.join(THREAD.to_string()).join("engine").exists());
    let branches = ["branch", "--list", "anima/*"];
    assert_eq!(bench.git_in(&bench.project, &branches).await, "");
    let entries = bench.project.join(".git/worktrees");
    let left = std::fs::read_dir(entries).map_or(0, Iterator::count);
    assert_eq!(left, 0, "an entry of the worktree is left");
    // The next try, with a launcher that works, shows both steps and makes the worktree.
    steps.lock().expect("the steps").clear();
    let made = bench
        .engine
        .worktree_shown(bench.plan(THREAD), Some(&shown));
    made.await.expect("a worktree");
    assert_eq!(
        *steps.lock().expect("the steps"),
        [Preparing::History, Preparing::Files]
    );
}

#[tokio::test]
async fn the_copy_of_a_project_is_removed_only_when_no_thread_has_a_worktree_of_it() {
    let bench = Bench::new().await;
    bench.worktree().await;
    let held = bench.engine.forget_project(&bench.project);
    assert!(
        matches!(&held, Err(GitRefusal::Failed { said }) if said.contains("remove the worktree")),
        "{held:?}"
    );
    let removed = bench.call(PERSON, GitAction::Remove).await;
    assert!(matches!(removed, GitOutcome::Removed { .. }), "{removed:?}");
    let used = bench.engine.disk();
    assert_eq!((used.len(), used[0].worktrees), (1, 0));
    let freed = bench
        .engine
        .forget_project(&bench.project)
        .expect("the removal");
    assert_eq!(freed, used[0].bytes);
    assert_eq!(bench.engine.disk(), []);
    // The repository of the person is as it was, and a new thread copies the history again.
    assert_eq!(bench.person_sees().await.0, "main");
    bench.worktree().await;
    let status = bench.engine.call(PERSON, target(), GitAction::Status).await;
    assert!(matches!(status, GitOutcome::Status(_)), "{status:?}");
}

#[tokio::test]
async fn a_partial_clone_is_refused_with_plain_words_and_a_shallow_clone_works() {
    let bench = Bench::new().await;
    // The second commit, so that a clone with one commit is shallow.
    std::fs::write(bench.project.join("a.txt"), "two\n").expect("a file");
    let second = ["commit", "--quiet", "--all", "--message=second"];
    bench.git_in(&bench.project, &second).await;
    let root = bench.home.parent().expect("the root");
    let shallow = root.join("shallow");
    let from = format!("file://{}", bench.project.display());
    let place = shallow.to_string_lossy().into_owned();
    let clone = [
        "clone",
        "--quiet",
        "--depth=1",
        from.as_str(),
        place.as_str(),
    ];
    bench.git_in(root, &clone).await;
    let plan = |thread| worktree::Plan {
        thread,
        project: &shallow,
        base: None,
    };
    let made = bench.engine.worktree(plan(Uuid::from_u128(11))).await;
    let made = made.expect("a worktree of a shallow clone");
    let text = std::fs::read_to_string(made.worktree.folder().join("a.txt")).expect("the file");
    assert_eq!(text, "two\n");
    let of = Target {
        thread: Uuid::from_u128(11),
        repository: "shallow",
    };
    let status = bench.engine.call(PERSON, of, GitAction::Status).await;
    assert!(matches!(status, GitOutcome::Status(_)), "{status:?}");
    // What a partial clone has in its store: a pack that the remote promised the rest of.
    std::fs::write(shallow.join(".git/objects/pack/pack-1.promisor"), "").expect("a file");
    let partial = bench.engine.worktree(plan(Uuid::from_u128(12))).await;
    assert!(
        matches!(&partial, Err(GitRefusal::Failed { said }) if said.contains("partial clone")),
        "{partial:?}"
    );
    assert!(refused(&GitOutcome::Trusted).is_none());
}
