//! Recovery across an upgrade after W1 saved a complete removal snapshot.

use agent_threads::git::{GitAction, GitOutcome};

use super::{Bench, PERSON, own_of};
use crate::threads::git::tests::THREAD;

#[tokio::test]
async fn interrupted_w1_removal_preserves_its_complete_ref_with_partial_or_absent_tree() {
    for absent in [false, true] {
        let bench = Bench::new().await;
        let folder = bench.worktree().await;
        let kept = crate::threads::git::checkpoint::kept_ref(THREAD);
        let original = bench.git_in(&bench.project, &["rev-parse", "HEAD"]).await;
        bench
            .git_in(&bench.project, &["update-ref", &kept, &original])
            .await;
        std::fs::write(own_of(&bench).join("anima-saved"), &original).unwrap();
        if absent {
            std::fs::remove_dir_all(&folder).unwrap();
        } else {
            std::fs::remove_file(folder.join("a.txt")).unwrap();
        }
        let obstruction = if absent {
            let receipt = own_of(&bench).with_extension("removed");
            std::fs::create_dir(&receipt).unwrap();
            receipt
        } else {
            let lock = bench.project.join(".git").join(format!("{kept}.lock"));
            std::fs::write(&lock, "another writer").unwrap();
            lock
        };
        let refused = bench.call(PERSON, GitAction::Remove).await;
        assert!(matches!(refused, GitOutcome::Refused { .. }), "{refused:?}");
        assert_eq!(
            bench.git_in(&bench.project, &["rev-parse", &kept]).await,
            original
        );
        assert!(own_of(&bench).join("anima-saved").is_file());
        if absent {
            std::fs::remove_dir(obstruction).unwrap();
        } else {
            std::fs::remove_file(obstruction).unwrap();
        }
        let outcome = bench.call(PERSON, GitAction::Remove).await;
        let GitOutcome::Removed {
            kept: Some(ref reported),
            ..
        } = outcome
        else {
            panic!("legacy recovery failed: {outcome:?}");
        };
        assert_eq!(reported, &kept);
        assert_eq!(
            bench.git_in(&bench.project, &["rev-parse", &kept]).await,
            original
        );
        assert_eq!(
            bench
                .git_in(&bench.project, &["show", &format!("{kept}:a.txt")])
                .await,
            "one"
        );
        assert_eq!(bench.call(PERSON, GitAction::Remove).await, outcome);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_snapshot_keeps_its_records_until_pack_and_lfs_directories_sync() {
    use crate::threads::git::place::walk::sync_probe;

    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let kept = crate::threads::git::checkpoint::kept_ref(THREAD);
    let original = bench.git_in(&bench.project, &["rev-parse", "HEAD"]).await;
    bench
        .git_in(&bench.project, &["update-ref", &kept, &original])
        .await;
    bench.git_in(&bench.project, &["repack", "-ad"]).await;
    let own = own_of(&bench);
    std::fs::write(own.join("anima-saved"), &original).unwrap();
    let common = bench.project.join(".git");
    let lfs = "lfs/objects/ab/cd";
    for root in [&own, &common] {
        let leaf = root.join(lfs);
        std::fs::create_dir_all(&leaf).unwrap();
        std::fs::write(leaf.join("abcd1234"), "fixture LFS content").unwrap();
    }
    let competing_lock = common.join(format!("{kept}.lock"));
    std::fs::write(&competing_lock, "another writer").unwrap();
    for dir in ["objects/pack", lfs] {
        let fault = sync_probe::fail_for(&common.join(dir), || {});
        let removal = bench.call(PERSON, GitAction::Remove).await;
        drop(fault);
        assert!(
            matches!(removal, GitOutcome::Refused { why: agent_threads::git::GitRefusal::Failed { said } }
            if said.contains("injected directory sync failure")),
            "{dir}"
        );
        assert!(own.join("anima-saved").is_file());
        assert!(folder.is_dir());
        assert!(!own.with_extension("removed").exists());
    }
    std::fs::remove_file(competing_lock).unwrap();
    let removal = bench.call(PERSON, GitAction::Remove).await;
    assert!(matches!(removal, GitOutcome::Removed { kept: Some(name), .. } if name == kept));
    assert_eq!(
        bench
            .git_in(&bench.project, &["show", &format!("{kept}:a.txt")])
            .await,
        "one"
    );
}
