use super::*;

#[tokio::test]
async fn creates_a_repository_with_no_templates_and_refuses_to_reinitialize_it() {
    let home = tempfile::tempdir().unwrap();
    let daemon = home.path().join("daemon");
    std::fs::create_dir(&daemon).unwrap();
    let requested = home.path().join("new project");
    let real = project(requested.clone(), daemon.clone()).await.unwrap();
    assert_eq!(real, std::fs::canonicalize(&requested).unwrap());
    assert_eq!(
        std::fs::read_to_string(real.join(".git/HEAD")).unwrap(),
        "ref: refs/heads/main\n"
    );
    assert!(!real.join(".git/hooks").exists());
    std::fs::write(real.join("keep.txt"), "keep this").unwrap();
    std::fs::write(real.join(".git/HEAD"), "ref: refs/heads/keep\n").unwrap();
    assert!(matches!(
        project(requested, daemon).await,
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
        assert!(project(path, daemon.clone()).await.is_err());
    }
    assert_eq!(std::fs::read_to_string(file).unwrap(), "original");
    assert!(!daemon.join(".git").exists());
    assert!(!daemon.join("new").exists());
    assert!(!home.path().join("absent").exists());
}
