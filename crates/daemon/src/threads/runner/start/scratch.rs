//! The folder of a thread with no project (THR-041): the person chose "No project", so the
//! runner makes a new empty folder for the thread under `~/Anima Scratch/`, outside the folder
//! of Anima. The folder counts as chosen by the person, and it stays when the thread is removed.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use agent_threads::control::Refusal;
use chrono::NaiveDate;

/// The folder below the home folder of the person that holds the folders of threads with no
/// project.
pub const SCRATCH: &str = "Anima Scratch";

/// Makes a new empty folder for a thread with no project, below the home folder that the login
/// environment `env` names, and gives its real path. Its name is the day and the random end of
/// a new ID, so two threads do not share a folder.
///
/// # Errors
/// [`Refusal::FolderNotPermitted`] when the login names no home folder, when `~/Anima Scratch`
/// is a link or not a folder (an agent may have put a link there), or when the new folder cannot
/// be made or is not where it was made.
pub fn made(env: &[(String, String)], today: NaiveDate) -> Result<PathBuf, Refusal> {
    let home = env
        .iter()
        .find(|(name, _)| name == "HOME")
        .map(|(_, home)| Path::new(home))
        .filter(|home| home.is_absolute())
        .ok_or(Refusal::FolderNotPermitted)?;
    let above = home.join(SCRATCH);
    match std::fs::symlink_metadata(&above) {
        Ok(found) if found.file_type().is_dir() => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            std::fs::create_dir(&above).map_err(|_| Refusal::FolderNotPermitted)?;
        }
        _ => return Err(Refusal::FolderNotPermitted),
    }
    let random = uuid::Uuid::now_v7().simple().to_string();
    let folder = above.join(format!("{today}-{}", &random[24..]));
    std::fs::create_dir(&folder).map_err(|_| Refusal::FolderNotPermitted)?;
    let real = std::fs::canonicalize(&folder).map_err(|_| Refusal::FolderNotPermitted)?;
    // The real home, not the real `above`: a link put in place of `above` after the check
    // would move both.
    let real_above = std::fs::canonicalize(home)
        .map_err(|_| Refusal::FolderNotPermitted)?
        .join(SCRATCH);
    if real.parent() == Some(real_above.as_path()) {
        Ok(real)
    } else {
        unmade(&folder);
        Err(Refusal::FolderNotPermitted)
    }
}

/// Removes a folder that [`made`] made for a start that was then refused: only while it is
/// still empty, so that nothing in it is lost.
pub fn unmade(folder: &Path) {
    let _ = std::fs::remove_dir(folder);
}

#[cfg(test)]
mod tests {
    use super::{SCRATCH, made};
    use agent_threads::control::Refusal;
    use chrono::NaiveDate;

    fn login(home: &std::path::Path) -> Vec<(String, String)> {
        vec![("HOME".to_owned(), home.display().to_string())]
    }

    const DAY: NaiveDate = NaiveDate::from_ymd_opt(2026, 11, 22).expect("a day");

    #[test]
    fn thr_041_each_thread_with_no_project_gets_a_new_empty_folder() {
        let home = tempfile::tempdir().expect("a folder");
        let first = made(&login(home.path()), DAY).expect("a folder");
        let second = made(&login(home.path()), DAY).expect("a folder");
        assert_ne!(first, second);
        let above = std::fs::canonicalize(home.path().join(SCRATCH)).expect("made");
        for folder in [&first, &second] {
            assert_eq!(folder.parent(), Some(above.as_path()));
            assert!(
                std::fs::read_dir(folder)
                    .expect("a folder")
                    .next()
                    .is_none()
            );
        }
        assert!(
            first
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("2026-11-22-"))
        );
    }

    #[test]
    fn thr_041_a_link_in_place_of_the_scratch_folder_is_refused() {
        let home = tempfile::tempdir().expect("a folder");
        let elsewhere = home.path().join("repo");
        std::fs::create_dir(&elsewhere).expect("a folder");
        std::os::unix::fs::symlink(&elsewhere, home.path().join(SCRATCH)).expect("a link");
        assert_eq!(
            made(&login(home.path()), DAY),
            Err(Refusal::FolderNotPermitted)
        );
        assert!(
            std::fs::read_dir(&elsewhere)
                .expect("a folder")
                .next()
                .is_none()
        );
        assert_eq!(made(&[], DAY), Err(Refusal::FolderNotPermitted), "no home");
    }
}
