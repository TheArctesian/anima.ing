use super::*;

#[test]
fn worker_progress_cannot_project_arbitrary_text_or_follow_a_symlink() {
    let work = std::env::temp_dir().join(format!("anima-progress-{}", uuid::Uuid::now_v7()));
    fs::create_dir(&work).unwrap();
    assert!(observe(&work).unwrap().is_none());
    write(&work, Stage::Detecting, false).unwrap();
    failed(&work).unwrap();
    let observed = serde_json::to_value(observe(&work).unwrap().unwrap()).unwrap();
    assert_eq!(observed["stage"], "detecting");
    assert_eq!(observed["failed"], true);
    let path = work.join("progress.json");
    for invalid in [
        r#"{"schema":1,"stage":"arbitrary private output","failed":false}"#,
        r#"{"schema":1,"stage":"building","failed":false,"log":"private output"}"#,
        r#"{"schema":2,"stage":"building","failed":false}"#,
    ] {
        fs::write(&path, invalid).unwrap();
        assert!(observe(&work).is_err());
    }
    write(&work, Stage::Building, false).unwrap();
    fs::rename(&path, work.join("elsewhere.json")).unwrap();
    std::os::unix::fs::symlink(work.join("elsewhere.json"), &path).unwrap();
    assert!(observe(&work).is_err());
    fs::remove_dir_all(work).unwrap();
}
