use super::*;

#[test]
fn a_failed_active_registry_release_keeps_recovery_and_can_be_retried() {
    let temp = tempfile::tempdir().expect("temporary home");
    let root = temp.path().canonicalize().expect("real home");
    let worktrees = root.join("worktrees");
    let records = root.join("records");
    std::fs::create_dir(&worktrees).expect("worktree home");
    super::super::super::place::private_folder(&records).expect("private records");
    let home = Home::at(&worktrees, &records).expect("record store");
    let thread = Uuid::now_v7();
    let repository = "fixture";
    let folder = home.planned(thread, repository).expect("worktree path");
    let own = home.own(thread, repository).expect("own Git path");
    let common = root.join("project.git");
    for dir in [&folder, &own, &common] {
        std::fs::create_dir_all(dir).expect("fixture folder");
    }
    std::fs::write(folder.join("draft"), "work").expect("worktree file");
    let record = Record {
        project: root.join("project"),
        home: worktrees,
        folder_id: FolderId::of(&folder),
        folder,
        git_dir: common.join("worktrees").join("fixture"),
        common_dir: common,
        own,
        shared: root.join("shared"),
        branches: Vec::new(),
        trusted: None,
    };
    home.keep(thread, repository, &record)
        .expect("active record");
    let retirement = Retirement::prepare(&home, &record, "refs/anima/kept/fixture".to_owned())
        .expect("durable intention");
    let active = record.own.with_extension("json");
    let original = active.with_extension("saved-record");
    std::fs::rename(&active, &original).expect("set record aside");
    std::fs::create_dir(&active).expect("obstruct release");
    let expected = retirement
        .container(&record, (thread, repository))
        .join("worktree");
    let first = retirement.finish(&home, (thread, repository), &record);
    assert!(
        matches!(first, Err(GitRefusal::Failed { said }) if said.contains(&expected.display().to_string()))
    );
    assert_eq!(
        std::fs::read_to_string(expected.join("draft")).expect("recoverable work"),
        "work"
    );
    assert!(!record.folder.exists());

    std::fs::remove_dir(&active).expect("clear obstruction");
    std::fs::rename(original, active).expect("restore active record for retry");
    let pending = Retirement::existing(&home, &record)
        .expect("read journal")
        .expect("pending release");
    let recovered = pending
        .finish(&home, (thread, repository), &record)
        .expect("finish retirement");
    assert!(
        matches!(recovered, GitOutcome::Removed { recovery: Some(path), .. } if path == expected)
    );
    assert!(home.recorded(thread, repository).is_err());
}

#[test]
fn tc7_archiving_git_records_keeps_an_occupied_recovery_directory() {
    let temp = tempfile::tempdir().expect("temporary records");
    let base = temp.path().canonicalize().expect("real records");
    let id = Uuid::now_v7();
    let active = base.join("active.git");
    let retained = base.join(format!("retired-{id}.git"));
    std::fs::create_dir(&active).expect("active record");
    std::fs::create_dir(&retained).expect("existing empty recovery record");
    std::fs::write(active.join("HEAD"), "work").expect("one active file");
    let home = Home::at(&base.with_file_name("worktrees"), &base).expect("home");
    assert!(archive(&home, &active, id).is_err());
    assert_eq!(
        std::fs::read_to_string(active.join("HEAD")).expect("active record untouched"),
        "work"
    );
    assert_eq!(
        std::fs::read_dir(retained)
            .expect("existing directory")
            .count(),
        0
    );
}
