use super::*;

#[tokio::test]
async fn abandoned_verifier_links_are_cleaned_without_touching_other_recovery_files() {
    use std::os::unix::fs::{MetadataExt as _, symlink};

    let bench = Bench::new().await;
    bench.worktree().await;
    let (kept, commit) = saved(&bench).await;
    let own = own_of(&bench);
    let abandoned = own.join("verify-retirement-Ab12Cd");
    let outside = bench.home.with_file_name("keep-outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("keep"), b"outside").unwrap();
    std::fs::create_dir(&abandoned).unwrap();
    let object = loose(&bench.project.join(".git/objects"), &commit);
    std::fs::hard_link(&object, abandoned.join("held-object")).unwrap();
    assert_eq!(std::fs::metadata(&object).unwrap().nlink(), 2);
    symlink(&outside, abandoned.join("outside")).unwrap();
    let other = own.join("verify-retirement-operator-notes");
    std::fs::create_dir(&other).unwrap();
    std::fs::write(other.join("keep"), b"operator evidence").unwrap();

    assert_legacy_release_replayed(&bench, &kept).await;
    assert_eq!(std::fs::metadata(&object).unwrap().nlink(), 1);
    let archive = own
        .parent()
        .unwrap()
        .read_dir()
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .find(|entry| entry.join("anima-saved").is_file())
        .expect("retained Git metadata");
    assert!(!archive.join(abandoned.file_name().unwrap()).exists());
    assert_eq!(
        std::fs::read(archive.join(other.file_name().unwrap()).join("keep")).unwrap(),
        b"operator evidence"
    );
    assert_eq!(std::fs::read(outside.join("keep")).unwrap(), b"outside");
}
