use std::os::unix::fs::{PermissionsExt as _, symlink};

use agent_threads::git::{GitAction, GitOutcome, GitRefusal};

use super::{Bench, PERSON};

#[tokio::test]
async fn pending_removal_releases_a_manually_moved_tree_with_its_saved_ref() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    std::fs::write(folder.join("draft.txt"), "saved draft\n").expect("the draft");
    let retirement = bench.home.join(".anima-retired");
    std::fs::create_dir(&retirement).expect("obstruct the recovery base");
    mode(&retirement, 0o755);

    let stopped = bench.call(PERSON, GitAction::Remove).await;
    let GitOutcome::Refused {
        why: GitRefusal::Failed { said },
    } = &stopped
    else {
        panic!("expected a refused move: {stopped:?}");
    };
    let said_original = said.contains("The worktree is still at")
        && said.contains(&folder.display().to_string())
        && !said.contains("recovery is at");
    let kept = format!("refs/anima/kept/{}", super::super::super::THREAD);
    assert_eq!(
        bench
            .git_in(&bench.project, &["show", &format!("{kept}:draft.txt")])
            .await,
        "saved draft"
    );

    mode(&retirement, 0o700);
    let moved = bench.home.with_file_name("moved-after-refusal");
    std::fs::rename(&folder, &moved).expect("manual move after refusal");
    symlink(&moved, &folder).expect("an ambiguous original location");
    let refused = bench.call(PERSON, GitAction::Remove).await;
    assert!(matches!(refused, GitOutcome::Refused { .. }), "{refused:?}");
    assert_eq!(bench.engine.disk()[0].worktrees, 1);
    std::fs::remove_file(&folder).expect("remove only the synthetic link");

    mode(&retirement, 0o755);
    let unknown = bench.call(PERSON, GitAction::Remove).await;
    assert!(
        matches!(&unknown, GitOutcome::Refused { why: GitRefusal::Failed { said } }
        if said.contains("The worktree location could not be verified.")),
        "{unknown:?}"
    );
    assert_eq!(bench.engine.disk()[0].worktrees, 1);
    mode(&retirement, 0o700);

    let outcome = bench.call(PERSON, GitAction::Remove).await;
    assert!(
        matches!(&outcome, GitOutcome::Removed {
        kept: Some(saved), recovery: None, original: Some(path)
    } if saved == &kept && path == &folder),
        "{outcome:?}"
    );
    assert!(said_original, "{said}");
    assert_eq!(bench.call(PERSON, GitAction::Remove).await, outcome);
    assert_eq!(bench.engine.disk()[0].worktrees, 0);
    assert_eq!(
        std::fs::read_to_string(moved.join("draft.txt")).expect("the untouched draft"),
        "saved draft\n"
    );
    bench
        .git_in(
            &bench.project,
            &["show-ref", "--verify", "refs/heads/anima/12345678"],
        )
        .await;
}

fn mode(path: &std::path::Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("recovery mode");
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn live_runner_refuses_an_acl_grant_in_the_records_path_and_on_the_receipt() {
    fn acl(path: &std::path::Path, arguments: &[&str]) {
        let status = std::process::Command::new("/bin/chmod")
            .env_clear()
            .args(arguments)
            .arg(path)
            .status()
            .expect("chmod");
        assert!(status.success(), "{status}");
    }

    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let own = super::own_of(&bench);
    let records = bench.home.with_file_name("records");
    for path in [records.as_path(), own.parent().expect("thread records")] {
        acl(
            path,
            &["+a", "everyone allow list,search,add_file,delete_child"],
        );
        let refused = bench.call(PERSON, GitAction::Remove).await;
        assert!(
            matches!(refused, GitOutcome::Refused { .. }),
            "{path:?}: {refused:?}"
        );
        assert!(folder.join("a.txt").is_file());
        acl(path, &["-N"]);
    }
    let removed = bench.call(PERSON, GitAction::Remove).await;
    assert!(matches!(removed, GitOutcome::Removed { .. }), "{removed:?}");
    let receipt = own.with_extension("removed");
    acl(&receipt, &["+a", "everyone allow read,write"]);
    let refused = bench.call(PERSON, GitAction::Remove).await;
    assert!(matches!(refused, GitOutcome::Refused { .. }), "{refused:?}");
    acl(&receipt, &["-N"]);
    assert_eq!(bench.call(PERSON, GitAction::Remove).await, removed);
}
