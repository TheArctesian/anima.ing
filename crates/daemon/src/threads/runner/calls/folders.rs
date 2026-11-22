//! The folders in one folder of this computer (TH113), so that the person can choose a project
//! by browsing, as T3 Code does. The runner reads with the rights of the person, as each program
//! of the person can. It gives names of folders only: never a file, never a link, never what a
//! folder below holds. The folder must be one that a thread could start in (THR-041), so the
//! folder of the daemon is not listed. The read is off the loop of the runner and has a time
//! limit, as a folder can be on a network that does not answer.

use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_threads::control::{Outcome, Refusal};
use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags};

use super::super::Runner;

/// The most entries that one read looks at, kept or not.
const MOST_LOOKED: usize = 20_000;

/// The most folders that one answer gives; `more` says that there were more.
const MOST_LISTED: usize = 1_000;

/// The longest name that the answer gives, in bytes (the limit of most file systems).
const LONGEST_NAME: usize = 255;

/// The longest wait for one read.
const READ_WAIT: Duration = Duration::from_secs(2);

impl Runner {
    /// The call `folders`.
    pub(super) async fn folders(&self, folder: PathBuf, hidden: bool) -> Outcome {
        let daemon = self.dir.parent().unwrap_or(&self.dir).to_owned();
        let read = tokio::task::spawn_blocking(move || listed(&folder, &daemon, hidden));
        match tokio::time::timeout(READ_WAIT, read).await {
            Ok(Ok(outcome)) => outcome,
            _ => Outcome::Refused {
                why: Refusal::FolderNotPermitted,
            },
        }
    }
}

/// The answer for `folder`: refused as a start in it would be refused (THR-041).
fn listed(folder: &Path, daemon: &Path, hidden: bool) -> Outcome {
    let real = match super::super::start::chosen_folder(folder, daemon) {
        Ok(real) => real,
        Err(why) => return Outcome::Refused { why },
    };
    let Ok(open) = rustix::fs::open(
        &real,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) else {
        return Outcome::Refused {
            why: Refusal::FolderNotPermitted,
        };
    };
    let (mut folders, more) = names(&open, hidden);
    folders.sort_by_key(|name| name.to_lowercase());
    Outcome::Folders {
        folder: real,
        folders,
        more,
    }
}

/// The names of the folders in the open folder `here`: no link, no file, no name that is not
/// UTF-8 or holds a control character, and a hidden one only when `hidden`. At most
/// [`MOST_LISTED`] of them, from at most [`MOST_LOOKED`] entries; `true` when there were more.
fn names(here: &rustix::fd::OwnedFd, hidden: bool) -> (Vec<String>, bool) {
    let Ok(read) = Dir::read_from(here) else {
        return (Vec::new(), false);
    };
    let mut names = Vec::new();
    for (looked, entry) in read.enumerate() {
        if looked == MOST_LOOKED || names.len() == MOST_LISTED {
            return (names, true);
        }
        let Ok(entry) = entry else { continue };
        let Some(name) = entry.file_name().to_str().ok() else {
            continue;
        };
        let shown = name != "."
            && name != ".."
            && name.len() <= LONGEST_NAME
            && (hidden || !name.starts_with('.'))
            && !name.chars().any(char::is_control);
        if shown && is_folder(here, name, entry.file_type()) {
            names.push(name.to_owned());
        }
    }
    (names, false)
}

/// Whether the entry is a folder, and not a link to one.
fn is_folder(here: &rustix::fd::OwnedFd, name: &str, kind: FileType) -> bool {
    let kind = match kind {
        FileType::Unknown => rustix::fs::statat(here, name, AtFlags::SYMLINK_NOFOLLOW)
            .map_or(FileType::Unknown, |stat| {
                FileType::from_raw_mode(stat.st_mode)
            }),
        known => known,
    };
    kind == FileType::Directory
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn th113_a_folder_lists_its_folders_only_and_never_through_a_link() {
        let home = tempfile::tempdir().expect("a folder");
        let home = std::fs::canonicalize(home.path()).expect("a real path");
        let outside = tempfile::tempdir().expect("a folder");
        for folder in ["b-project", "A-project", ".hidden", "daemon/threads"] {
            std::fs::create_dir_all(home.join(folder)).expect("a folder");
        }
        std::fs::write(home.join("notes.txt"), "text").expect("a file");
        std::os::unix::fs::symlink(outside.path(), home.join("linked")).expect("a link");
        let daemon = home.join("daemon");

        let Outcome::Folders {
            folder,
            folders,
            more,
        } = listed(&home, &daemon, false)
        else {
            panic!("a listing");
        };
        assert_eq!(folder, home);
        assert_eq!(folders, ["A-project", "b-project", "daemon"]);
        assert!(!more);
        let Outcome::Folders { folders, .. } = listed(&home, &daemon, true) else {
            panic!("a listing");
        };
        assert!(folders.contains(&".hidden".to_owned()), "{folders:?}");

        for refused in [
            daemon.clone(),
            daemon.join("threads"),
            home.join("notes.txt"),
            PathBuf::from("relative"),
            home.join("missing"),
        ] {
            assert_eq!(
                listed(&refused, &daemon, false),
                Outcome::Refused {
                    why: Refusal::FolderNotPermitted
                },
                "{}",
                refused.display()
            );
        }
    }

    #[test]
    fn th113_a_large_folder_gives_its_first_folders_and_says_there_are_more() {
        let home = tempfile::tempdir().expect("a folder");
        for index in 0..=MOST_LISTED {
            std::fs::create_dir(home.path().join(format!("f{index:04}"))).expect("a folder");
        }
        let Outcome::Folders { folders, more, .. } =
            listed(home.path(), Path::new("/nonexistent/daemon"), false)
        else {
            panic!("a listing");
        };
        assert_eq!(folders.len(), MOST_LISTED);
        assert!(more);
    }
}
