//! What is written in the repository of the person while or before the runner works (the
//! design fault of the fourth security check): a hook or a setting that appears while an
//! action runs, a hook with a name that is not text, a file that the configuration takes in
//! only inside the new worktree, a rewrite of the URL of a push.

use std::fmt::Write as _;
use std::io::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_threads::git::{GitAction, GitOutcome, GitRefusal};
use uuid::Uuid;

use super::trusted::{HOOKS, PUSH, UNCOMMITTED, commit};
use super::{Bench, PERSON, refused};

/// Writes new hooks and settings into the configuration of the project again and again, as a
/// process that an agent left behind can, until `stop`. Each one would make the file `ran`.
fn keeps_planting(project: &std::path::Path, ran: &std::path::Path, stop: &Arc<AtomicBool>) {
    let config = project.join(".git/config");
    let clean = std::fs::read_to_string(&config).expect("the configuration");
    let touch = format!("touch \"{}\"", ran.display());
    let mut round = 0_u32;
    while !stop.load(Ordering::Relaxed) {
        round += 1;
        let mut planted = clean.clone();
        for event in HOOKS {
            let _ = writeln!(
                planted,
                "[hook \"r{round}-{event}\"]\n\tevent = {event}\n\tcommand = {touch}"
            );
        }
        let _ = write!(
            planted,
            "[core]\n\tfsmonitor = {touch}\n\tsshCommand = {touch}\n\thooksPath = /tmp/r{round}\n\
             [filter \"r{round}\"]\n\tclean = {touch}; cat\n\tsmudge = {touch}; cat\n\
             [diff]\n\texternal = {touch}\n[credential]\n\thelper = !{touch}\n\
             [gpg]\n\tprogram = {touch}\n[commit]\n\tgpgSign = true\n"
        );
        // Whole at each moment, as git writes its configuration: a new file moved in place.
        let next = config.with_extension("planted");
        std::fs::write(&next, planted).expect("the planted configuration");
        std::fs::rename(&next, &config).expect("the move");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hook_or_a_setting_written_while_an_action_runs_starts_no_program() {
    let bench = Bench::new().await;
    bench.with_origin().await;
    let folder = bench.worktree().await;
    let ran = folder.with_file_name("RAN");
    bench.hook(&format!("touch \"{}\"", ran.display()));
    std::fs::write(folder.join(".gitattributes"), "*.txt filter=r1\n").expect("a file");
    let stop = Arc::new(AtomicBool::new(false));
    let planting = {
        let (project, ran, stop) = (bench.project.clone(), ran.clone(), Arc::clone(&stop));
        std::thread::spawn(move || keeps_planting(&project, &ran, &stop))
    };
    for round in 1..=3_u32 {
        std::fs::write(folder.join("a.txt"), format!("round {round}\n")).expect("a file");
        assert!(matches!(
            bench.call(PERSON, GitAction::Status).await,
            GitOutcome::Status(_)
        ));
        bench.turn_ended(round).await;
        let committed = bench.call(PERSON, commit("", None)).await;
        assert!(
            matches!(committed, GitOutcome::Committed { .. }),
            "{committed:?}"
        );
        let pushed = bench.call(PERSON, PUSH).await;
        assert!(matches!(pushed, GitOutcome::Pushed { .. }), "{pushed:?}");
        let reverted = bench.call(PERSON, GitAction::Revert { turn: 0 }).await;
        assert!(
            matches!(reverted, GitOutcome::Reverted { .. }),
            "{reverted:?}"
        );
    }
    // A worktree of a different thread is made and removed while the planting goes on.
    let other = Uuid::from_u128(0xabcd_ef01_2345_6789_abcd_ef01_2345_6789);
    let plan = super::worktree::Plan {
        thread: other,
        project: &bench.project,
        base: Some("main"),
    };
    bench.engine.worktree(plan).await.expect("a worktree");
    let target = crate::threads::git::Target {
        thread: other,
        repository: "engine",
    };
    let removed = bench.engine.call(PERSON, target, GitAction::Remove).await;
    assert!(matches!(removed, GitOutcome::Removed { .. }), "{removed:?}");
    stop.store(true, Ordering::Relaxed);
    planting.join().expect("the planting");
    assert!(!ran.exists(), "a program of the repository ran");
    let origin = bench.project.with_file_name("origin.git");
    let sent = ["log", "-1", "--format=%an", "anima/12345678"];
    assert_eq!(bench.git_in(&origin, &sent).await, "Ada");
}

#[tokio::test]
async fn a_hook_with_a_name_that_is_not_text_runs_for_nothing() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let ran = folder.with_file_name("RAN");
    let mut planted = b"[hook \"\xff\xfe\"]\n\tevent = reference-transaction\n".to_vec();
    planted.extend(format!("\tcommand = touch \"{}\"\n", ran.display()).bytes());
    let config = bench.project.join(".git/config");
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(config)
        .expect("the configuration");
    file.write_all(&planted).expect("the planted hook");
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");

    // The checkpoints and the changes do not read the configuration of the repository.
    assert!(matches!(
        bench.call(PERSON, UNCOMMITTED).await,
        GitOutcome::Changes(_)
    ));
    bench.turn_ended(1).await;
    // A commit reads who the person is from it, as text; it is refused while it is not text.
    let committed = bench.call(PERSON, commit("", None)).await;
    assert!(
        matches!(refused(&committed), Some(GitRefusal::Failed { said }) if said.contains("not text")),
        "{committed:?}"
    );
    assert!(!ran.exists(), "the hook ran");
}

#[tokio::test]
async fn a_file_that_the_configuration_takes_in_only_in_the_new_worktree_runs_no_hook() {
    let bench = Bench::new().await;
    let root = bench.home.parent().expect("the root");
    let ran = root.join("RAN");
    let taken_in = root.join("planted.inc");
    let mut hooks = String::new();
    for event in HOOKS {
        let _ = writeln!(
            hooks,
            "[hook \"late-{event}\"]\n\tevent = {event}\n\tcommand = touch \"{}\"",
            ran.display()
        );
    }
    std::fs::write(&taken_in, hooks).expect("the file");
    // The project itself is on `main`, so its own listing does not have the hooks.
    let when = "includeIf.onbranch:anima/**.path";
    let path = taken_in.to_string_lossy().into_owned();
    bench.git_in(&bench.project, &["config", when, &path]).await;
    bench.hook(&format!("touch \"{}\"", ran.display()));
    let _ = std::fs::remove_file(&ran);
    let folder = bench.worktree().await;
    assert!(folder.join("a.txt").exists(), "the files were checked out");
    assert!(!ran.exists(), "a hook ran when the worktree was made");
}

#[tokio::test]
async fn a_push_goes_only_where_the_person_accepted_whatever_the_repository_rewrites() {
    let bench = Bench::new().await;
    bench.with_origin().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    let committed = bench.call(PERSON, commit("", None)).await;
    assert!(
        matches!(committed, GitOutcome::Committed { .. }),
        "{committed:?}"
    );
    // A second place, and each way the repository has to send a push there.
    let origin = bench.project.with_file_name("origin.git");
    let elsewhere = bench.project.with_file_name("elsewhere.git");
    let place = elsewhere.to_string_lossy().into_owned();
    let bare = ["init", "--quiet", "--bare", place.as_str()];
    bench.git_in(&bench.project, &bare).await;
    let rewrite = format!("url.{place}.insteadOf");
    let rewrite_push = format!("url.{place}.pushInsteadOf");
    let from = origin.to_string_lossy().into_owned();
    for key in [rewrite.as_str(), rewrite_push.as_str()] {
        bench.git_in(&folder, &["config", key, &from]).await;
    }
    let pushed = bench.call(PERSON, PUSH).await;
    assert!(matches!(pushed, GitOutcome::Pushed { .. }), "{pushed:?}");
    let branches = ["for-each-ref", "--format=%(refname:short)", "refs/heads"];
    assert_eq!(bench.git_in(&elsewhere, &branches).await, "");
    assert!(
        bench
            .git_in(&origin, &branches)
            .await
            .contains("anima/12345678")
    );
    // A new URL for the push is named, and nothing is sent until the person accepts it.
    std::fs::write(folder.join("a.txt"), "changed again\n").expect("a file");
    assert_eq!(bench.trust(PERSON).await, GitOutcome::Trusted);
    let pushurl = ["config", "remote.origin.pushurl", place.as_str()];
    bench.git_in(&folder, &pushurl).await;
    let changed = bench.status(PERSON).await.changed;
    assert!(
        changed.iter().any(|name| name.contains("elsewhere.git")),
        "{changed:?}"
    );
    for action in [commit("", None), PUSH] {
        let outcome = bench.call(PERSON, action).await;
        assert!(
            matches!(refused(&outcome), Some(GitRefusal::Changed { .. })),
            "{outcome:?}"
        );
    }
    assert_eq!(bench.git_in(&elsewhere, &branches).await, "");
}
