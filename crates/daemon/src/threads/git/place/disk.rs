//! The disk that the Git of a project uses in the folder of the records, and the removal of a
//! project: the checked copy of its history ([`super::shared`]) is as large as the Git folder
//! of the project, once for each project. A person sees the size and removes the copy of a
//! project that has no thread with a worktree any more.
//!
//! I/O: reads the sizes of files and removes folders of the runner.

use std::path::{Path, PathBuf};

use agent_threads::git::GitRefusal;

use super::super::Engine;
use super::Record;

/// The file in the copy of a project that says which repository it is of.
const OF: &str = "anima-project";

/// What the Git of one project uses on the disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Used {
    /// The repository of the person.
    pub project: PathBuf,
    /// The bytes of the copy of its history and of the Git folders of its worktrees.
    pub bytes: u64,
    /// The threads that have a worktree of it.
    pub worktrees: usize,
}

/// Writes down in the copy at `shared` which repository it is of.
///
/// # Errors
/// The error of the file system.
pub fn name(shared: &Path, project: &Path) -> std::io::Result<()> {
    std::fs::write(shared.join(OF), project.as_os_str().as_encoded_bytes())
}

/// The bytes of each file below `folder`; a link is counted as a link, never followed.
fn bytes(folder: &Path) -> u64 {
    let Ok(listed) = std::fs::read_dir(folder) else {
        return 0;
    };
    listed
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => bytes(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |about| about.len()),
            _ => 0,
        })
        .sum()
}

impl Engine {
    /// The record of each worktree that the runner made; `None` for one that cannot be read.
    fn records(&self) -> Vec<Option<Record>> {
        let Ok(threads) = std::fs::read_dir(self.home.records()) else {
            return Vec::new();
        };
        let files = threads
            .flatten()
            // The folder of the projects holds no record of a worktree.
            .filter(|thread| thread.path() != self.home.projects())
            .filter_map(|thread| std::fs::read_dir(thread.path()).ok())
            .flat_map(|files| files.flatten().map(|file| file.path()));
        files
            .filter(|file| file.extension().is_some_and(|kind| kind == "json"))
            .map(|file| serde_json::from_slice(&std::fs::read(file).ok()?).ok())
            .collect()
    }

    /// What each project uses on the disk, the largest first.
    #[must_use]
    pub fn disk(&self) -> Vec<Used> {
        let records: Vec<Record> = self.records().into_iter().flatten().collect();
        let Ok(copies) = std::fs::read_dir(self.home.projects()) else {
            return Vec::new();
        };
        let mut used: Vec<Used> = copies
            .flatten()
            .filter_map(|copy| {
                let project = std::fs::read(copy.path().join(OF)).ok()?;
                // The bytes are the ones that `name` wrote from a path of this system.
                let project = PathBuf::from(String::from_utf8(project).ok()?);
                let of_it: Vec<&Record> = records
                    .iter()
                    .filter(|record| record.project == project)
                    .collect();
                let own: u64 = of_it.iter().map(|record| bytes(&record.own)).sum();
                Some(Used {
                    bytes: bytes(&copy.path()) + own,
                    worktrees: of_it.len(),
                    project,
                })
            })
            .collect();
        used.sort_by_key(|used| std::cmp::Reverse(used.bytes));
        used
    }

    /// Removes what the runner keeps for the repository at `project`: the checked copy of its
    /// history and what the person accepted for it. Gives the bytes that are free again. The
    /// repository of the person is not changed.
    ///
    /// # Errors
    /// A [`GitRefusal`] while a thread has a worktree of the project (remove the worktree
    /// first), while a thread holds the records, or when a folder cannot be removed.
    pub fn forget_project(&self, project: &Path) -> Result<u64, GitRefusal> {
        self.closed()?;
        // The same spelling of the path as when the worktrees were made.
        let real = std::fs::canonicalize(project);
        let project = real.as_deref().unwrap_or(project);
        // A record that cannot be read can be of this project: nothing is removed then.
        let uses = |record: &Option<Record>| {
            record
                .as_ref()
                .is_none_or(|record| record.project == project)
        };
        if self.records().iter().any(uses) {
            return Err(GitRefusal::Failed {
                said: "a chat still has a worktree of this project; remove the worktree first"
                    .to_owned(),
            });
        }
        let copy = self.home.shared(project);
        let freed = bytes(&copy);
        let gone = |failed: std::io::Error| GitRefusal::Failed {
            said: format!("the copy of the project could not be removed: {failed}"),
        };
        match std::fs::remove_dir_all(&copy) {
            Err(failed) if failed.kind() != std::io::ErrorKind::NotFound => {
                return Err(gone(failed));
            }
            _ => {}
        }
        // What the person accepted for the project goes with it.
        let _ = std::fs::remove_file(copy.with_extension("json"));
        Ok(freed)
    }
}
