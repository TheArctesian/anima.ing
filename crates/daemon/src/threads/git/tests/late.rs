//! What the listing of the configuration could miss (fourth security check): a name that is
//! not text, a file that the configuration takes in only inside the new worktree, and a
//! changed setting behind more hook files than the page shows.

use std::io::Write as _;

use agent_threads::git::{GitOutcome, GitRefusal};

use super::trusted::{PUSH, UNCOMMITTED, commit};
use super::{Bench, PERSON, refused};

#[tokio::test]
async fn a_hook_with_a_name_that_is_not_text_stops_each_run() {
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

    let shown = bench.call(PERSON, UNCOMMITTED).await;
    assert!(
        matches!(refused(&shown), Some(GitRefusal::Failed { said }) if said.contains("not text")),
        "{shown:?}"
    );
    let target = super::target();
    assert!(bench.engine.turn_ended(target, 1).await.is_err());
    assert!(!ran.exists(), "the hook ran");
}

#[tokio::test]
async fn a_file_that_the_configuration_takes_in_only_in_the_new_worktree_runs_no_hook() {
    let bench = Bench::new().await;
    let root = bench.home.parent().expect("the root");
    let ran = root.join("RAN");
    let taken_in = root.join("planted.inc");
    let hook = format!(
        "[hook \"late\"]\n\tevent = post-index-change\n\tcommand = touch \"{}\"\n",
        ran.display()
    );
    std::fs::write(&taken_in, hook).expect("the file");
    // The project itself is on `main`, so its own listing does not have the hook.
    let when = "includeIf.onbranch:anima/**.path";
    let path = taken_in.to_string_lossy().into_owned();
    bench.git_in(&bench.project, &["config", when, &path]).await;
    let _ = std::fs::remove_file(&ran);
    let folder = bench.worktree().await;
    assert!(folder.join("a.txt").exists(), "the files were checked out");
    assert!(!ran.exists(), "the hook ran when the worktree was made");
}

#[tokio::test]
async fn many_hook_files_do_not_hide_a_setting_that_sends_a_push_elsewhere() {
    let bench = Bench::new().await;
    bench.with_origin().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("a.txt"), "changed\n").expect("a file");
    let hooks = bench.project.join(".git/hooks");
    std::fs::create_dir_all(&hooks).expect("the hooks");
    for at in 0..45 {
        std::fs::write(hooks.join(format!("a-{at}")), "x\n").expect("a file");
    }
    let elsewhere = bench.project.with_file_name("elsewhere.git");
    let elsewhere = elsewhere.to_string_lossy().into_owned();
    let pushurl = ["config", "remote.origin.pushurl", elsewhere.as_str()];
    bench.git_in(&folder, &pushurl).await;

    assert_eq!(
        bench.status(PERSON).await.changed,
        ["remote.origin.pushurl"]
    );
    for action in [commit("", None), PUSH] {
        let outcome = bench.call(PERSON, action).await;
        assert!(
            matches!(refused(&outcome), Some(GitRefusal::Changed { what }) if what.len() == 1),
            "{outcome:?}"
        );
    }
    // A rewrite of URLs cannot be accepted at all: a push is refused while it is there.
    let rewrite = [
        "config",
        "url.https://example.invalid/.insteadOf",
        "https://",
    ];
    bench.git_in(&folder, &rewrite).await;
    assert_eq!(bench.trust(PERSON).await, GitOutcome::Trusted);
    let outcome = bench.call(PERSON, PUSH).await;
    assert!(
        matches!(refused(&outcome), Some(GitRefusal::Failed { said }) if said.contains("insteadof")),
        "{outcome:?}"
    );
}
