//! The `@` list of the composer (TH108): the files and folders of the working folder of a thread
//! that the runner started. The walk stays in that folder: it follows no link, so no path of the
//! answer is outside it, also when a name is changed into a link during the walk. It leaves out what [`files::left_out`] names and what the `.gitignore`
//! files on the way leave out; it opens no other file. A walk is kept a few seconds, because the
//! composer asks again after each key.

use std::collections::{BTreeMap, VecDeque};
use std::os::fd::OwnedFd;
use std::path::Path;
use std::time::{Duration, Instant};

use agent_threads::control::{Outcome, Refusal};
use agent_threads::files::{self, FoundPath, Ignored, MAX_QUERY, MOST_FOUND};
use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags};
use uuid::Uuid;

use super::super::Runner;

/// The most entries that one walk looks at, kept or not: a folder with more lists the first
/// ones from the top.
const MOST_WALKED: usize = 20_000;

/// The deepest folder that the walk goes into, below the working folder.
const DEEPEST: usize = 16;

/// How long a walk answers again before the folder is walked again.
const KEPT: Duration = Duration::from_secs(5);

/// The most bytes of a `.gitignore` that the walk reads; a longer one is read to there.
const MAX_IGNORE_FILE: u64 = 256 * 1024;

/// The last walk of the folder of each thread.
#[derive(Debug, Default)]
pub struct Walked(BTreeMap<Uuid, (Instant, Vec<FoundPath>)>);

impl Runner {
    /// The paths of the folder of `thread` that match `query`, the best first.
    pub(super) fn files(&mut self, thread: Uuid, query: &str, most: usize) -> Outcome {
        if query.len() > MAX_QUERY || most == 0 || most > MOST_FOUND {
            return Outcome::Refused {
                why: Refusal::BadValue,
            };
        }
        let Some(started) = self.threads.get(&thread) else {
            return Outcome::Refused {
                why: Refusal::UnknownThread,
            };
        };
        let now = Instant::now();
        let Walked(kept) = &mut self.walked;
        kept.retain(|id, (at, _)| self.threads.contains_key(id) && now - *at < KEPT);
        let (_, found) = kept
            .entry(thread)
            .or_insert_with(|| (now, walk(&started.row.folder)));
        let (paths, more) = files::ranked(found, query, most);
        Outcome::Files { paths, more }
    }
}

/// The files and folders below `root`, folder by folder from the top, each folder in the order
/// of its names. Each folder is opened from `root` one name at a time, and no name may be a link,
/// so a name that becomes a link during the walk is passed over and never leads out. A folder
/// that cannot be opened is passed over. At most [`MOST_WALKED`] entries are looked at.
fn walk(root: &Path) -> Vec<FoundPath> {
    let mut found = Vec::new();
    let Ok(top) = rustix::fs::open(root, opened(true), Mode::empty()) else {
        return found;
    };
    let exclude = ignore_text(&top, &[".git", "info", "exclude"]);
    let mut budget = MOST_WALKED;
    let mut folders =
        VecDeque::from([(Vec::<String>::new(), Ignored::default().with("", &exclude))]);
    while let Some((parts, ignored)) = folders.pop_front() {
        let Some(here) = folder_at(&top, &parts) else {
            continue;
        };
        let base = parts.join("/");
        let ignored = ignored.with(&base, &ignore_text(&here, &[".gitignore"]));
        for (name, folder) in entries(&here, &mut budget) {
            let path = if base.is_empty() {
                name.clone()
            } else {
                format!("{base}/{name}")
            };
            if files::left_out(&path, folder) || ignored.leaves_out(&path, folder) {
                continue;
            }
            if folder && parts.len() < DEEPEST {
                let mut below = parts.clone();
                below.push(name);
                folders.push_back((below, ignored.clone()));
            }
            found.push(FoundPath { path, folder });
        }
        if budget == 0 {
            break;
        }
    }
    found
}

/// The flags that open a folder (or a file) by one name and refuse a link.
fn opened(folder: bool) -> OFlags {
    let kind = if folder {
        OFlags::DIRECTORY
    } else {
        OFlags::empty()
    };
    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | kind
}

/// The folder at `parts` below `top`, opened one name at a time with no link followed.
fn folder_at(top: &OwnedFd, parts: &[String]) -> Option<OwnedFd> {
    let mut here = rustix::fs::openat(top, ".", opened(true), Mode::empty()).ok()?;
    for part in parts {
        here = rustix::fs::openat(&here, part.as_str(), opened(true), Mode::empty()).ok()?;
    }
    Some(here)
}

/// The plain files and folders of the open folder `here`, by name, each name a valid UTF-8
/// name with no backslash. Each entry looked at, kept or not, takes one from `budget`.
fn entries(here: &OwnedFd, budget: &mut usize) -> Vec<(String, bool)> {
    let Ok(read) = Dir::read_from(here) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for entry in read {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let Ok(entry) = entry else { continue };
        let Some(name) = entry.file_name().to_str().ok().map(str::to_owned) else {
            continue;
        };
        if name == "." || name == ".." || name.contains('\\') {
            continue;
        }
        let kind = match entry.file_type() {
            FileType::Unknown => rustix::fs::statat(here, name.as_str(), AtFlags::SYMLINK_NOFOLLOW)
                .map_or(FileType::Unknown, |stat| {
                    FileType::from_raw_mode(stat.st_mode)
                }),
            known => known,
        };
        match kind {
            FileType::RegularFile => entries.push((name, false)),
            FileType::Directory => entries.push((name, true)),
            _ => {}
        }
    }
    entries.sort();
    entries
}

/// The text of the plain file at `parts` below `folder`, opened with no link followed, at most
/// [`MAX_IGNORE_FILE`] bytes; else nothing. A file that is not UTF-8 gives nothing.
fn ignore_text(folder: &OwnedFd, parts: &[&str]) -> String {
    use std::io::Read as _;
    let Some((name, folders)) = parts.split_last() else {
        return String::new();
    };
    let mut here = None;
    for part in folders {
        let from = here.as_ref().unwrap_or(folder);
        match rustix::fs::openat(from, *part, opened(true), Mode::empty()) {
            Ok(next) => here = Some(next),
            Err(_) => return String::new(),
        }
    }
    let from = here.as_ref().unwrap_or(folder);
    let Ok(fd) = rustix::fs::openat(from, *name, opened(false) | OFlags::NONBLOCK, Mode::empty())
    else {
        return String::new();
    };
    let file = std::fs::File::from(fd);
    let mut text = String::new();
    if file.metadata().is_ok_and(|meta| meta.is_file()) {
        let _ = file.take(MAX_IGNORE_FILE).read_to_string(&mut text);
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn th108_the_walk_lists_from_the_top_and_stays_in_the_folder() {
        let root = tempfile::tempdir().expect("folder");
        let outside = tempfile::tempdir().expect("folder");
        let at = |path: &str| root.path().join(path);
        std::fs::create_dir_all(at("src/deep")).expect("folders");
        std::fs::create_dir_all(at("node_modules/pkg")).expect("folders");
        std::fs::create_dir_all(at("secrets")).expect("folders");
        for file in [
            "README.md",
            "src/main.rs",
            "src/deep/x.rs",
            ".env",
            "node_modules/pkg/i.js",
            "secrets/k.toml",
        ] {
            std::fs::write(at(file), "").expect("file");
        }
        std::fs::write(at(".gitignore"), "*.log\n").expect("file");
        std::fs::write(at("src/.gitignore"), "/deep/\n").expect("file");
        std::fs::write(at("src/run.log"), "").expect("file");
        std::fs::write(outside.path().join("far.txt"), "").expect("file");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.path(), at("away")).expect("link");
            std::os::unix::fs::symlink(outside.path().join("far.txt"), at("far.txt"))
                .expect("link");
        }

        let walked: Vec<(String, bool)> = walk(root.path())
            .into_iter()
            .map(|found| (found.path, found.folder))
            .collect();
        assert_eq!(
            walked,
            [
                (".gitignore".to_owned(), false),
                ("README.md".to_owned(), false),
                ("src".to_owned(), true),
                ("src/.gitignore".to_owned(), false),
                ("src/main.rs".to_owned(), false),
            ]
        );
    }

    #[test]
    fn th108_a_folder_that_became_a_link_is_not_opened() {
        let root = tempfile::tempdir().expect("folder");
        let outside = tempfile::tempdir().expect("folder");
        std::fs::create_dir(root.path().join("src")).expect("folder");
        let top = rustix::fs::open(root.path(), opened(true), Mode::empty()).expect("open");
        assert!(folder_at(&top, &["src".to_owned()]).is_some());
        // The agent of the thread swaps the folder for a link after the walk listed it.
        std::fs::remove_dir(root.path().join("src")).expect("removed");
        std::os::unix::fs::symlink(outside.path(), root.path().join("src")).expect("link");
        assert!(folder_at(&top, &["src".to_owned()]).is_none());
    }

    #[test]
    fn th108_only_the_person_asks_the_files_of_a_thread() {
        use agent_threads::control::Action;
        let thread = Uuid::now_v7();
        assert!(super::super::for_the_person(&Action::Files {
            thread,
            query: String::new(),
            most: 10,
        }));
        assert!(!super::super::for_the_person(&Action::Earlier {
            thread,
            before: 1,
            most: 10,
        }));
    }
}
