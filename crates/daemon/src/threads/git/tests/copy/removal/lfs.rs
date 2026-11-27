use agent_threads::git::{GitAction, GitOutcome};
use sha2::Digest as _;

use super::{Bench, PERSON, own_of};

#[tokio::test]
async fn fresh_removal_never_reuses_or_publishes_corrupt_lfs_payloads() {
    let bench = Bench::new().await;
    let folder = bench.worktree().await;
    let correct = b"expected LFS payload";
    let wrong = b"expected LFS payloaD";
    let oid = crate::model::fetch::hex(&sha2::Sha256::digest(correct));
    let name = format!("lfs/objects/{}/{}/{oid}", &oid[..2], &oid[2..4]);
    let original = own_of(&bench).join(&name);
    let project = bench.project.join(".git").join(name);
    for path in [&original, &project] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, wrong).unwrap();
    }
    std::fs::write(folder.join("draft.txt"), b"keep the worktree").unwrap();

    let refused = bench.call(PERSON, GitAction::Remove).await;
    assert!(matches!(refused, GitOutcome::Refused { .. }), "{refused:?}");
    assert!(folder.join("draft.txt").is_file());
    assert!(own_of(&bench).with_extension("json").is_file());
    assert_eq!(std::fs::read(&project).unwrap(), wrong);
    std::fs::write(&original, correct).unwrap();

    let removed = bench.call(PERSON, GitAction::Remove).await;
    assert!(matches!(removed, GitOutcome::Removed { .. }), "{removed:?}");
    assert_eq!(std::fs::read(project).unwrap(), correct);
}
