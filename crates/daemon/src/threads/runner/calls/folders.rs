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
use super::super::socket::Caller;

/// The most entries that one read looks at, kept or not.
const MOST_LOOKED: usize = 20_000;

/// The most folders that one answer gives; `more` says that there were more.
const MOST_LISTED: usize = 1_000;

/// The longest name that the answer gives, in bytes (the limit of most file systems).
const LONGEST_NAME: usize = 255;

/// The longest wait for one read; the call is refused after it.
const READ_WAIT: Duration = Duration::from_secs(2);

/// The reads that may run at one time. A read that hangs (a network folder that does not
/// answer) keeps its place until it ends, so hung reads never take more threads than this.
static READS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(3);

impl Runner {
    /// The call `folders`: the read runs away from the loop of the runner, which goes on; the
    /// client gets the answer when it ends.
    pub(super) fn folders(&self, folder: PathBuf, hidden: bool, back: Caller) {
        let daemon = self.dir.parent().unwrap_or(&self.dir).to_owned();
        let refused = || Outcome::Refused {
            why: Refusal::FolderNotPermitted,
        };
        let Ok(place) = READS.try_acquire() else {
            back.answer(refused());
            return;
        };
        tokio::spawn(async move {
            let read = tokio::task::spawn_blocking(move || {
                let _place = place;
                listed(&folder, &daemon, hidden)
            });
            let outcome = match tokio::time::timeout(READ_WAIT, read).await {
                Ok(Ok(outcome)) => outcome,
                _ => refused(),
            };
            back.answer(outcome);
        });
    }
}

/// The answer for `folder`: refused as a start in it would be refused (THR-041).
fn listed(folder: &Path, daemon: &Path, hidden: bool) -> Outcome {
    let refused = Outcome::Refused {
        why: Refusal::FolderNotPermitted,
    };
    let real = match super::super::start::chosen_folder(folder, daemon) {
        Ok(real) => real,
        Err(why) => return Outcome::Refused { why },
    };
    let Ok(open) = rustix::fs::open(
        &real,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) else {
        return refused;
    };
    // A folder of the path may have been changed into a link after the check: the folder that
    // opened must be the one that was checked.
    if opened_path(&open).as_deref() != Some(real.as_path()) {
        return refused;
    }
    let (folders, more) = names(&open, hidden);
    Outcome::Folders {
        folder: real,
        folders,
        more,
    }
}

/// The path of an open folder, as the system says it now.
fn opened_path(open: &rustix::fd::OwnedFd) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStringExt as _;
        let path = rustix::fs::getpath(open).ok()?;
        Some(PathBuf::from(std::ffi::OsString::from_vec(
            path.into_bytes(),
        )))
    }
    #[cfg(not(target_os = "macos"))]
    {
        use std::os::fd::AsRawFd as _;
        std::fs::read_link(format!("/proc/self/fd/{}", open.as_raw_fd())).ok()
    }
}

/// Whether a character is shown as itself: not a control character, and not one that changes
/// how the text around it looks (a direction mark, a joiner of no width), so that a name cannot
/// pass for a different one.
fn plain(character: char) -> bool {
    !character.is_control()
        && !matches!(
            character,
            '\u{00AD}'
                | '\u{061C}'
                | '\u{180E}'
                | '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{206F}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
        )
}

/// The names of the folders in the open folder `here`, sorted with no regard to case: no link,
/// no file, no name that is not UTF-8 or that holds a character that is not [`plain`], and a
/// hidden one only when `hidden`. At most [`MOST_LISTED`] of them, from at most [`MOST_LOOKED`]
/// entries; `true` when there were more folders, or entries that were not looked at.
fn names(here: &rustix::fd::OwnedFd, hidden: bool) -> (Vec<String>, bool) {
    let Ok(read) = Dir::read_from(here) else {
        return (Vec::new(), false);
    };
    let mut names = Vec::new();
    let mut more = false;
    for (looked, entry) in read.enumerate() {
        if looked == MOST_LOOKED {
            more = true;
            break;
        }
        let Ok(entry) = entry else { continue };
        let Some(name) = entry.file_name().to_str().ok() else {
            continue;
        };
        let shown = name != "."
            && name != ".."
            && name.len() <= LONGEST_NAME
            && (hidden || !name.starts_with('.'))
            && name.chars().all(plain);
        if shown && is_folder(here, name, entry.file_type()) {
            names.push(name.to_owned());
        }
    }
    names.sort_by_key(|name| name.to_lowercase());
    if names.len() > MOST_LISTED {
        names.truncate(MOST_LISTED);
        more = true;
    }
    (names, more)
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
        std::fs::create_dir(home.join("\u{202E}tcejorp")).expect("a folder with a direction mark");
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
