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
    let (thread, repository) = (Uuid::now_v7(), "fixture");
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
        orphan_removal: None,
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

    std::fs::remove_dir(&active).expect("clear obstruction");
    std::fs::rename(original, active).expect("restore active record for retry");
    check_source_parent_retry(&expected, &home, (thread, repository), &record);
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

fn check_source_parent_retry(recovery: &Path, home: &Home, target: (Uuid, &str), record: &Record) {
    let source = record.folder.parent().unwrap();
    std::fs::remove_dir(source).expect("the source parent is empty after moving");
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), source).unwrap();
    let pending = Retirement::existing(home, record).unwrap().unwrap();
    assert!(
        pending.finish(home, target, record).is_err(),
        "a source link is not absence"
    );
    std::fs::remove_file(source).unwrap();
    assert!(recovery.join("draft").is_file());
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
