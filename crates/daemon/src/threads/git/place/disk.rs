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
use super::records::Directory;

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
    /// The bytes of the checkpoints of the thread that uses the most; one thread can have
    /// `place::own::ROOM`, or what the person set.
    pub largest_thread: u64,
}

/// Writes down in the copy at `shared` which repository it is of.
///
/// # Errors
/// The error of the file system.
pub fn name(shared: &Path, project: &Path) -> std::io::Result<()> {
    std::fs::write(shared.join(OF), project.as_os_str().as_encoded_bytes())
}

/// The bytes of the disk that the files and folders below `folder` use; a link is not
/// followed and counts as nothing.
pub(super) fn bytes(folder: &Path) -> u64 {
    let Ok(listed) = std::fs::read_dir(folder) else {
        return 0;
    };
    listed
        .flatten()
        .map(|entry| match (entry.file_type(), entry.metadata()) {
            (Ok(kind), Ok(about)) if kind.is_dir() => on_disk(&about) + bytes(&entry.path()),
            (Ok(kind), Ok(about)) if kind.is_file() => on_disk(&about),
            _ => 0,
        })
        .fold(0, u64::saturating_add)
}

/// The bytes of the disk that a file or a folder of the runner uses: its blocks, which is
/// more than its length for a small file. Many small files use the disk by their count.
#[cfg(unix)]
fn on_disk(about: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt as _;
    about.blocks().saturating_mul(512).max(about.len())
}

#[cfg(not(unix))]
fn on_disk(about: &std::fs::Metadata) -> u64 {
    about.len()
}

impl Engine {
    fn records(&self) -> std::io::Result<Vec<Record>> {
        let root = self.home.records();
        let mut records = Vec::new();
        for thread in Directory::open(root, root, false)?.names()? {
            let Some(thread) = thread
                .to_str()
                .and_then(|name| uuid::Uuid::parse_str(name).ok())
            else {
                continue;
            };
            let folder = root.join(thread.to_string());
            let entries = match Directory::open(root, &folder, false) {
                Ok(entries) => entries,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            for file in entries.names()? {
                if Path::new(&file)
                    .extension()
                    .is_some_and(|kind| kind == "json")
                    && let Some(bytes) = entries.read(&file)?
                {
                    records.push(serde_json::from_slice(&bytes)?);
                }
            }
        }
        Ok(records)
    }

    /// What each project uses on the disk, the largest first.
    #[must_use]
    pub fn disk(&self) -> Vec<Used> {
        // While a thread holds the records, nothing in them is read.
        if self.closed().is_err() {
            return Vec::new();
        }
        let Ok(records) = self.records() else {
            return Vec::new();
        };
        let root = self.home.records();
        let projects = self.home.projects();
        let copies = Directory::open(root, &projects, false).and_then(|folder| folder.names());
        let Ok(copies) = copies else {
            return Vec::new();
        };
        let mut used: Vec<Used> = copies
            .into_iter()
            .filter(|copy| Path::new(copy).extension().is_some_and(|ext| ext == "git"))
            .filter_map(|copy| {
                let path = projects.join(copy);
                let directory = Directory::open(root, &path, false).ok()?;
                let project = directory.read(OF.as_ref()).ok()??;
                // The bytes are the ones that `name` wrote from a path of this system.
                let project = PathBuf::from(String::from_utf8(project).ok()?);
                let of_it: Vec<&Record> = records
                    .iter()
                    .filter(|record| record.project == project)
                    .collect();
                let own: Vec<u64> = of_it
                    .iter()
                    .filter(|record| self.home.private_git_exists(&record.own).unwrap_or(false))
                    .map(|record| bytes(&record.own))
                    .collect();
                Some(Used {
                    bytes: bytes(&path) + own.iter().sum::<u64>(),
                    largest_thread: own.iter().copied().max().unwrap_or(0),
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
        let failed = |error| GitRefusal::Failed {
            said: format!("the copy of the project could not be removed: {error}"),
        };
        if self
            .records()
            .map_err(failed)?
            .iter()
            .any(|record| record.project == project)
        {
            return Err(GitRefusal::Failed {
                said: "a chat still has a worktree of this project; remove the worktree first"
                    .to_owned(),
            });
        }
        let copy = self.home.shared(project);
        let parent = match self.home.state_parent(&copy, false) {
            Ok(parent) => parent,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(failed(error)),
        };
        let freed = if self.home.private_git_exists(&copy).map_err(failed)? {
            bytes(&copy)
        } else {
            0
        };
        let name = copy.file_name().ok_or(GitRefusal::BadPath)?;
        let baseline = copy.with_extension("json");
        self.home.read_state(&baseline).map_err(failed)?;
        parent.remove_tree(name).map_err(failed)?;
        self.home.remove_state(&baseline).map_err(failed)?;
        Ok(freed)
    }
}
