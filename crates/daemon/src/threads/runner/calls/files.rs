//! The `@` list of the composer (TH108): the files and folders of the working folder of a thread
//! that the runner started. The walk stays in that folder: it follows no link, so no path of the
//! answer is outside it, and it leaves out what [`files::left_out`] names. It opens no file.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use agent_threads::control::{Outcome, Refusal};
use agent_threads::files::{self, FoundPath, MAX_QUERY};
use uuid::Uuid;

use super::super::Runner;

/// The most paths that one walk finds: a folder with more lists the first ones from the top.
const MOST_WALKED: usize = 20_000;

/// The deepest folder that the walk goes into, below the working folder.
const DEEPEST: usize = 16;

impl Runner {
    /// The paths of the folder of `thread` that match `query`, the best first.
    pub(super) fn files(&self, thread: Uuid, query: &str, most: usize) -> Outcome {
        if query.len() > MAX_QUERY {
            return Outcome::Refused {
                why: Refusal::BadValue,
            };
        }
        let Some(started) = self.threads.get(&thread) else {
            return Outcome::Refused {
                why: Refusal::UnknownThread,
            };
        };
        Outcome::Files {
            paths: files::ranked(&walk(&started.row.folder), query, most),
        }
    }
}

/// The files and folders below `root`, folder by folder from the top, each folder in the order
/// of its names. A folder that cannot be read is passed over.
fn walk(root: &Path) -> Vec<FoundPath> {
    let mut found = Vec::new();
    let mut folders = VecDeque::from([(PathBuf::new(), 0)]);
    while let Some((below, depth)) = folders.pop_front() {
        let Ok(read) = std::fs::read_dir(root.join(&below)) else {
            continue;
        };
        let mut entries: Vec<_> = read.filter_map(Result::ok).collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            // The type of the entry itself: a link is neither a file nor a folder here.
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if kind.is_symlink() || !(kind.is_file() || kind.is_dir()) {
                continue;
            }
            let relative = below.join(&name);
            let Some(path) = relative.to_str().map(|path| path.replace('\\', "/")) else {
                continue;
            };
            let folder = kind.is_dir();
            if files::left_out(&path, folder) {
                continue;
            }
            if folder && depth < DEEPEST {
                folders.push_back((relative, depth + 1));
            }
            found.push(FoundPath { path, folder });
            if found.len() >= MOST_WALKED {
                return found;
            }
        }
    }
    found
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
                ("README.md".to_owned(), false),
                ("src".to_owned(), true),
                ("src/deep".to_owned(), true),
                ("src/main.rs".to_owned(), false),
                ("src/deep/x.rs".to_owned(), false),
            ]
        );
    }
}
