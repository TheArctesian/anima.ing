//! The folder of a thread with no project (THR-041): the person chose "No project", so the
//! runner makes a new empty folder for the thread under `~/Anima Scratch/`, outside the folder
//! of Anima. The folder counts as chosen by the person, and it stays when the thread is removed.

use std::path::{Path, PathBuf};

use agent_threads::control::Refusal;
use chrono::NaiveDate;

/// The folder below the home folder of the person that holds the folders of threads with no
/// project.
pub const SCRATCH: &str = "Anima Scratch";

/// Makes a new empty folder for a thread with no project, below the home folder that the login
/// environment `env` names, and gives its path. Its name is the day and the random end of a
/// new ID, so two threads do not share a folder.
///
/// # Errors
/// [`Refusal::FolderNotPermitted`] when the login names no home folder or the folder cannot be
/// made.
pub fn made(env: &[(String, String)], today: NaiveDate) -> Result<PathBuf, Refusal> {
    let home = env
        .iter()
        .find(|(name, _)| name == "HOME")
        .map(|(_, home)| Path::new(home))
        .filter(|home| home.is_absolute())
        .ok_or(Refusal::FolderNotPermitted)?;
    let above = home.join(SCRATCH);
    std::fs::create_dir_all(&above).map_err(|_| Refusal::FolderNotPermitted)?;
    let random = uuid::Uuid::now_v7().simple().to_string();
    let folder = above.join(format!("{today}-{}", &random[24..]));
    std::fs::create_dir(&folder).map_err(|_| Refusal::FolderNotPermitted)?;
    std::fs::canonicalize(&folder).map_err(|_| Refusal::FolderNotPermitted)
}
