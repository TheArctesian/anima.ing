//! The checked place of the records, and which folders that an agent can write hold them.

use std::os::unix::fs::PermissionsExt as _;

use super::*;

fn above() -> (tempfile::TempDir, PathBuf) {
    let keep = tempfile::tempdir().expect("a folder");
    let real = std::fs::canonicalize(keep.path()).expect("its real path");
    (keep, real)
}

fn mode(path: &Path, mode: u32) {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("a mode");
}

#[test]
fn the_folder_is_made_for_its_owner_alone_and_found_again() {
    let (_keep, above) = above();
    let place = above.join("anima-501");
    assert_eq!(opened(&place), Ok(place.clone()));
    let about = std::fs::metadata(&place).expect("the folder");
    assert_eq!(about.permissions().mode() & 0o777, 0o700);
    assert_eq!(opened(&place), Ok(place));
    assert_eq!(
        standard(501).and_then(|place| place.file_name().map(ToOwned::to_owned)),
        Some("anima-501".into())
    );
}

#[test]
fn a_folder_open_to_others_a_link_and_a_file_are_never_used() {
    let (_keep, above) = above();
    let open = above.join("open");
    std::fs::create_dir(&open).expect("a folder");
    mode(&open, 0o750);
    assert_eq!(opened(&open), Err(BadRecords::OpenToOthers));
    // What a different user can leave at the place first: a link to a folder of ours.
    let ours = above.join("ours");
    std::fs::create_dir(&ours).expect("a folder");
    mode(&ours, 0o700);
    let link = above.join("anima-link");
    std::os::unix::fs::symlink(&ours, &link).expect("a link");
    assert_eq!(opened(&link), Err(BadRecords::NotAFolder));
    assert_eq!(checked(&link), Err(BadRecords::NotAFolder));
    assert_eq!(checked(&above.join("nothing")), Err(BadRecords::Gone));
    let file = above.join("anima-file");
    std::fs::write(&file, "").expect("a file");
    assert_eq!(opened(&file), Err(BadRecords::NotAFolder));
    assert_eq!(opened(Path::new("relative")), Err(BadRecords::BadPath));
}

#[test]
fn a_folder_above_that_others_can_change_is_refused_unless_entries_are_kept() {
    let (_keep, above) = above();
    let shared = above.join("shared");
    std::fs::create_dir(&shared).expect("a folder");
    mode(&shared, 0o777);
    assert_eq!(
        opened(&shared.join("anima-501")),
        Err(BadRecords::OpenAbove)
    );
    // As `/Users/Shared` and `/var/tmp`: each user writes, only the owner removes.
    mode(&shared, 0o1777);
    assert!(opened(&shared.join("anima-501")).is_ok());
    let through = above.join("through");
    std::os::unix::fs::symlink(&shared, &through).expect("a link");
    assert_eq!(
        opened(&through.join("anima-502")),
        Err(BadRecords::OpenAbove)
    );
}

#[test]
fn a_touch_marks_each_file_of_the_records_as_used_now() {
    let (_keep, above) = above();
    let place = opened(&above.join("anima-501")).expect("the records");
    std::fs::create_dir_all(place.join("a/b")).expect("folders");
    let file = place.join("a/b/pack");
    std::fs::write(&file, "x").expect("a file");
    let long_ago = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_hours(24);
    let old = std::fs::FileTimes::new()
        .set_accessed(long_ago)
        .set_modified(long_ago);
    std::fs::File::options()
        .write(true)
        .open(&file)
        .and_then(|file| file.set_times(old))
        .expect("old times");
    touch(&place).expect("a touch");
    let now = std::fs::metadata(&file)
        .and_then(|about| about.modified())
        .expect("a time");
    assert!(now > long_ago + std::time::Duration::from_hours(24));
}

#[test]
fn a_thread_in_a_folder_above_the_records_holds_them() {
    let (_keep, above) = above();
    let records = above.join("shared/anima-501");
    std::fs::create_dir_all(&records).expect("a folder");
    std::fs::create_dir_all(above.join("home/project")).expect("a folder");
    assert!(held_by(&records, &above));
    assert!(held_by(&records, &above.join("shared")));
    assert!(held_by(&records, &records));
    std::fs::create_dir_all(records.join("inside")).expect("a folder");
    assert!(held_by(&records, &records.join("inside")));
    assert!(!held_by(&records, &above.join("home")));
    assert!(!held_by(&records, &above.join("home/project")));
    assert!(held_by(&records, &above.join("gone")));
    std::os::unix::fs::symlink(above.join("shared"), above.join("home/link")).expect("a link");
    assert!(held_by(&records, &above.join("home/link")));
}

/// The records in `above/shared/anima-501`, a project of the person, and a private `TMPDIR`
/// of a thread in the folder of the runner, as the runner makes it.
fn records_and_roots(above: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let records = above.join("shared/anima-501");
    let project = above.join("home/project");
    let private = above.join("runner/tmp/thread");
    for folder in [&records, &project, &private] {
        std::fs::create_dir_all(folder).expect("a folder");
    }
    (records, project, private)
}

#[test]
fn a_thread_holds_the_records_when_its_folder_holds_them() {
    let (_keep, above) = above();
    let (records, project, private) = records_and_roots(&above);
    assert!(!held_by_any(&records, &[project.clone(), private.clone()]));
    assert!(held_by_any(&records, &[above.clone(), private.clone()]));
    assert!(held_by_any(&records, &[records.join("."), private]));
    assert!(!held_by_any(&records, &[]));
}

#[test]
fn a_thread_holds_the_records_when_its_tmpdir_holds_them() {
    let (_keep, above) = above();
    let (records, project, private) = records_and_roots(&above);
    // As `TMPDIR=/var/tmp` on Linux, where the records of each user are.
    assert!(held_by_any(
        &records,
        &[project.clone(), above.join("shared")]
    ));
    std::fs::create_dir_all(records.join("tmp")).expect("a folder");
    assert!(held_by_any(
        &records,
        &[project.clone(), records.join("tmp")]
    ));
    assert!(!held_by_any(&records, &[project, private]));
}

#[test]
fn a_thread_holds_the_records_when_they_are_in_tmp() {
    let keep = tempfile::tempdir_in("/tmp").expect("a folder in /tmp");
    let (records, project, private) = records_and_roots(keep.path());
    let roots = |tmp: &str| vec![project.clone(), private.clone(), PathBuf::from(tmp)];
    assert!(!held_by_any(&records, &[project.clone(), private.clone()]));
    assert!(held_by_any(&records, &roots("/tmp")));
    if cfg!(target_os = "macos") {
        // `/tmp` is a link to it there.
        assert!(held_by_any(&records, &roots("/private/tmp")));
    }
    // Records that are not made yet, below a link: the link is followed.
    let link = above_link(keep.path());
    assert!(held_by_any(&link.join("anima-502"), &roots("/tmp")));
    assert!(!held_by_any(&link.join("anima-502"), &[project, private]));
}

/// A link in `above` to `above/shared`.
fn above_link(above: &Path) -> PathBuf {
    let link = above.join("to-shared");
    std::os::unix::fs::symlink(above.join("shared"), &link).expect("a link");
    link
}

#[test]
fn a_thread_holds_the_records_when_a_more_root_of_its_sandbox_holds_them() {
    let (_keep, above) = above();
    let (records, project, private) = records_and_roots(&above);
    let with = |more: PathBuf| vec![project.clone(), private.clone(), more];
    assert!(!held_by_any(&records, &with(above.join("home"))));
    assert!(held_by_any(&records, &with(above.join("shared"))));
    std::os::unix::fs::symlink(above.join("shared"), above.join("home/shared")).expect("a link");
    assert!(held_by_any(&records, &with(above.join("home/shared"))));
    // A root that cannot be resolved counts as one that holds them.
    assert!(held_by_any(&records, &with(above.join("gone"))));
}
