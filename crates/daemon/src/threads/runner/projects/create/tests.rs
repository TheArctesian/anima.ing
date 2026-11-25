use super::*;

#[tokio::test]
async fn creates_a_repository_with_no_templates_and_refuses_to_reinitialize_it() {
    let home = tempfile::tempdir().unwrap();
    let daemon = home.path().join("daemon");
    std::fs::create_dir(&daemon).unwrap();
    let requested = home.path().join("new project");
    let prepared = project(requested.clone(), daemon.clone(), Preparation::Initialize)
        .await
        .unwrap();
    assert!(!requested.exists());
    let real = prepared.publish().unwrap().folder().unwrap();
    assert_eq!(real, std::fs::canonicalize(&requested).unwrap());
    assert_eq!(
        std::fs::read_to_string(real.join(".git/HEAD")).unwrap(),
        "ref: refs/heads/main\n"
    );
    assert!(!real.join(".git/hooks").exists());
    std::fs::write(real.join("keep.txt"), "keep this").unwrap();
    std::fs::write(real.join(".git/HEAD"), "ref: refs/heads/keep\n").unwrap();
    assert!(matches!(
        project(requested, daemon, Preparation::Initialize).await,
        Err(Refusal::CannotCreateProject { .. })
    ));
    assert_eq!(
        std::fs::read_to_string(real.join("keep.txt")).unwrap(),
        "keep this"
    );
    assert_eq!(
        std::fs::read_to_string(real.join(".git/HEAD")).unwrap(),
        "ref: refs/heads/keep\n"
    );
}

#[tokio::test]
async fn refuses_files_links_daemon_paths_and_missing_parents_without_changing_them() {
    let home = tempfile::tempdir().unwrap();
    let daemon = home.path().join("daemon");
    std::fs::create_dir(&daemon).unwrap();
    let file = home.path().join("file");
    std::fs::write(&file, "original").unwrap();
    let link = home.path().join("link");
    std::os::unix::fs::symlink(&daemon, &link).unwrap();
    for path in [
        file.clone(),
        link,
        daemon.join("new"),
        home.path().join("absent/new"),
        home.path().join("../escape"),
        PathBuf::from("relative"),
    ] {
        assert!(
            project(path, daemon.clone(), Preparation::Initialize)
                .await
                .is_err()
        );
    }
    assert_eq!(std::fs::read_to_string(file).unwrap(), "original");
    assert!(!daemon.join(".git").exists());
    assert!(!daemon.join("new").exists());
    assert!(!home.path().join("absent").exists());
}

#[tokio::test]
async fn replaced_parent_never_redirects_the_prepared_repository() {
    let home = tempfile::tempdir().unwrap();
    let parent = home.path().join("parent");
    let outside = home.path().join("outside");
    std::fs::create_dir(&parent).unwrap();
    std::fs::create_dir(&outside).unwrap();
    let parent = std::fs::canonicalize(parent).unwrap();
    let target = place::Destination::open(parent.join("new")).unwrap();
    let staged = home.path().join("staged");
    std::fs::create_dir(&staged).unwrap();
    initialize(&staged).await.unwrap();
    std::fs::rename(&parent, home.path().join("moved")).unwrap();
    std::os::unix::fs::symlink(&outside, &parent).unwrap();
    assert!(target.publish(&staged).is_err());
    assert!(!outside.join("new").exists());
    assert!(staged.join(".git/HEAD").exists());
}

#[tokio::test]
async fn a_destination_created_during_preparation_is_never_replaced() {
    let home = tempfile::tempdir().unwrap();
    let parent = std::fs::canonicalize(home.path()).unwrap();
    let requested = parent.join("new");
    let target = place::Destination::open(requested.clone()).unwrap();
    let staged = parent.join("staged");
    std::fs::create_dir(&staged).unwrap();
    initialize(&staged).await.unwrap();
    std::fs::create_dir(&requested).unwrap();
    assert!(target.publish(&staged).is_err());
    assert_eq!(std::fs::read_dir(&requested).unwrap().count(), 0);
    assert!(staged.join(".git/HEAD").exists());
}

#[tokio::test]
async fn a_replaced_project_is_not_registered_and_replacement_files_stay_unchanged() {
    let home = tempfile::tempdir().unwrap();
    let daemon = home.path().join("daemon");
    std::fs::create_dir(&daemon).unwrap();
    let project = project(home.path().join("new"), daemon, Preparation::Initialize)
        .await
        .unwrap()
        .publish()
        .unwrap();
    let path = project.folder().unwrap();
    std::fs::rename(&path, home.path().join("kept")).unwrap();
    let outside = home.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("keep"), "original").unwrap();
    std::os::unix::fs::symlink(&outside, path).unwrap();
    assert!(project.folder().is_err());
    assert_eq!(
        std::fs::read_to_string(outside.join("keep")).unwrap(),
        "original"
    );
    assert!(!outside.join(".git").exists());
}
