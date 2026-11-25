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
