//! Where the worktrees of the threads are, and the checks of each path and name before `git`
//! gets it (THR-041, THR-050): a name from a fixed alphabet, a path with no `..`, and a real
//! path that stays below the folder of the worktrees.
//!
//! The checks of text are pure; [`Home::worktree`] reads the file system.

pub mod chosen;
pub mod disk;
pub(super) mod lfs;
pub mod own;
pub mod records;
mod registry;
pub mod shared;
pub mod walk;
pub mod wired;

use std::path::{Component, Path, PathBuf};

use sha2::Digest as _;
use uuid::Uuid;

/// The prefix of the branch the runner makes for a thread.
pub const BRANCH_PREFIX: &str = "anima/";

/// The most bytes of a branch name or of a path inside a repository.
const MOST: usize = 240;

/// Why a path or a name was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BadPlace {
    #[error("the path leaves its folder")]
    Escapes,
    #[error("the folder is not a worktree that the runner made")]
    NotOurWorktree,
    #[error("the folder is the checkout of the person, which a thread never changes")]
    MainCheckout,
    #[error("the name has a character that is not allowed")]
    BadName,
    #[error("the worktree is there, but its record or its checkpoints are gone")]
    Expired,
    #[error("the folder is inside a repository")]
    InARepository,
    #[error("the folder is inside a folder of Anima or one that an agent can write, or holds one")]
    KeptOut,
}

/// The folder of the runner that holds the worktrees, `<home>/<thread>/<repository>`, and the
/// folder of its records, which is outside each repository and each worktree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Home {
    folder: PathBuf,
    records: PathBuf,
}

/// What the runner wrote down when it made a worktree, before an agent ran in it. Each later
/// run of `git` is held to these places and branches, whatever the files of the worktree say.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Record {
    /// The repository of the person the worktree was made from.
    pub project: PathBuf,
    /// The home of the worktrees when this one was made. A worktree stays there when the
    /// person chooses a different home later.
    pub home: PathBuf,
    /// The real folder of the worktree.
    pub folder: PathBuf,
    /// The Git folder of the worktree: `<common>/worktrees/<name>`.
    pub git_dir: PathBuf,
    /// The Git folder of the repository of the person.
    pub common_dir: PathBuf,
    /// The Git folder of the runner for this worktree ([`own`]): the one its `git` runs with.
    pub own: PathBuf,
    /// The checked copy of the history of the project ([`shared`]), which `own` borrows from.
    pub shared: PathBuf,
    /// The branches the runner made for this thread: the only ones a commit or a push may move.
    pub branches: Vec<String>,
    /// Who a commit is from and where a push goes, as the person last accepted them
    /// (`trust`).
    #[serde(default)]
    pub trusted: Option<std::collections::BTreeMap<String, String>>,
    /// What the work folder was when it was made; `None` where the system has no such number.
    #[serde(default)]
    pub folder_id: Option<super::run::folder::FolderId>,
    /// Binds an interrupted orphan removal to a legacy record without a folder identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) orphan_removal: Option<Uuid>,
}

/// A worktree of a thread that passed each check: its real folder is below the home and is the
/// one of its record, with a `.git` file (a linked worktree), never the checkout of the person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worktree {
    thread: Uuid,
    record: Record,
}

impl Worktree {
    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.record.folder
    }

    #[must_use]
    pub const fn thread(&self) -> Uuid {
        self.thread
    }

    #[must_use]
    pub const fn record(&self) -> &Record {
        &self.record
    }

    /// The Git folders of this worktree in the repository of the person.
    #[must_use]
    pub fn layout(&self) -> super::project::Layout {
        super::project::Layout {
            git_dir: self.record.git_dir.clone(),
            common_dir: self.record.common_dir.clone(),
        }
    }

    /// The runner made the branch `name` for this thread.
    #[must_use]
    pub fn owns(&self, name: &str) -> bool {
        self.record.branches.iter().any(|branch| branch == name)
    }
}

impl Home {
    /// The home at `folder` with its records in `records`; both absolute, neither in the other.
    ///
    /// # Errors
    /// [`BadPlace::Escapes`] for a relative path, one with `..`, or one inside the other.
    pub fn at(folder: &Path, records: &Path) -> Result<Self, BadPlace> {
        let good = |path: &Path| path.is_absolute() && plain(path);
        if good(folder)
            && good(records)
            && !records.starts_with(folder)
            && !folder.starts_with(records)
        {
            Ok(Self {
                folder: folder.to_owned(),
                records: records.to_owned(),
            })
        } else {
            Err(BadPlace::Escapes)
        }
    }

    #[must_use]
    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// The folder of the records, outside each repository and each worktree.
    #[must_use]
    pub fn records(&self) -> &Path {
        &self.records
    }

    /// The folder of one thread, which holds one worktree for each repository (THR-116).
    #[must_use]
    pub fn of_thread(&self, thread: Uuid) -> PathBuf {
        self.folder.join(thread.as_hyphenated().to_string())
    }

    /// Where the worktree of `repository` of `thread` is or will be. The name is checked; the
    /// folder is not read.
    ///
    /// # Errors
    /// [`BadPlace::BadName`] for a name outside the alphabet of [`folder_name`].
    pub fn planned(&self, thread: Uuid, repository: &str) -> Result<PathBuf, BadPlace> {
        if folder_name(repository) == repository {
            Ok(self.of_thread(thread).join(repository))
        } else {
            Err(BadPlace::BadName)
        }
    }

    /// Where the Git folder of the runner for the worktree of `repository` of `thread` is:
    /// in the folder of the records, where no agent writes.
    ///
    /// # Errors
    /// [`BadPlace::BadName`] for a name outside the alphabet of [`folder_name`].
    pub fn own(&self, thread: Uuid, repository: &str) -> Result<PathBuf, BadPlace> {
        self.planned(thread, repository)?;
        let thread = thread.as_hyphenated().to_string();
        Ok(self.records.join(thread).join(format!("{repository}.git")))
    }

    /// The folder of what the runner keeps for each project: the checked copy of its history
    /// and what the person accepted for it.
    #[must_use]
    pub fn projects(&self) -> PathBuf {
        self.records.join("projects")
    }

    /// Where the checked copy of the history of the repository at `project` is.
    #[must_use]
    pub fn shared(&self, project: &Path) -> PathBuf {
        let name = sha2::Sha256::digest(project.as_os_str().as_encoded_bytes());
        let name = crate::model::fetch::hex(&name);
        self.projects().join(format!("{name}.git"))
    }

    pub(super) fn record_file(&self, thread: Uuid, repository: &str) -> Result<PathBuf, BadPlace> {
        self.planned(thread, repository)?;
        let thread = thread.as_hyphenated().to_string();
        Ok(self.records.join(thread).join(format!("{repository}.json")))
    }

    /// The worktree of `repository` of `thread` for its removal alone, when it is not whole
    /// any more (its `.git` file or its entry in the repository of the person is gone): the
    /// record, the Git folder of the runner, and the folder at its planned place, which is
    /// still the folder that the runner made. The `git` of the runner needs no more to save
    /// the files.
    #[must_use]
    pub fn for_removal(&self, thread: Uuid, repository: &str) -> Option<Worktree> {
        let record = self.recorded(thread, repository).ok()?;
        let planned = Self::at(&record.home, &self.records)
            .and_then(|home| home.planned(thread, repository))
            .ok()?;
        let real = std::fs::canonicalize(planned).ok()?;
        let home = std::fs::canonicalize(&record.home).ok()?;
        let same = record.folder_id.is_none()
            || super::run::folder::FolderId::of(&real) == record.folder_id;
        let ours = real == record.folder
            && real.starts_with(&home)
            && real != home
            && same
            && record.own == self.own(thread, repository).ok()?
            && self.private_git_exists(&record.own).ok()?;
        ours.then_some(Worktree { thread, record })
    }

    /// The worktree of `repository` of `thread`, after each check: the runner has a record of
    /// it; the real path (each symlink followed) is the folder of the record and is below the
    /// real home; its `.git` is a file, as in a linked worktree; and its Git folder is below
    /// the `worktrees` folder of the repository it was made from.
    ///
    /// # Errors
    /// A [`BadPlace`] when a check fails.
    pub fn worktree(&self, thread: Uuid, repository: &str) -> Result<Worktree, BadPlace> {
        let record = match self.recorded(thread, repository) {
            Ok(record) => record,
            Err(_) if self.planned(thread, repository)?.join(".git").is_file() => {
                return Err(BadPlace::Expired);
            }
            Err(none) => return Err(none),
        };
        match self.private_git_exists(&record.own) {
            Ok(true) => {}
            Ok(false) => return Err(BadPlace::Expired),
            Err(_) => return Err(BadPlace::NotOurWorktree),
        }
        // Where the worktree was made, which is not the home of now after a change of it.
        let planned = Self::at(&record.home, &self.records)?.planned(thread, repository)?;
        let home = std::fs::canonicalize(&record.home).map_err(|_| BadPlace::NotOurWorktree)?;
        let real = std::fs::canonicalize(&planned).map_err(|_| BadPlace::NotOurWorktree)?;
        if !real.starts_with(&home) || real == home || real != record.folder {
            return Err(BadPlace::Escapes);
        }
        let dot_git = real.join(".git");
        let meta = std::fs::symlink_metadata(&dot_git).map_err(|_| BadPlace::NotOurWorktree)?;
        if meta.is_dir() {
            return Err(BadPlace::MainCheckout);
        }
        let git_dir =
            std::fs::canonicalize(&record.git_dir).map_err(|_| BadPlace::NotOurWorktree)?;
        let ours = git_dir.parent() == Some(record.common_dir.join("worktrees").as_path());
        if !meta.is_file() || !ours || git_dir != record.git_dir {
            return Err(BadPlace::NotOurWorktree);
        }
        let ours = record.own == self.own(thread, repository)?
            && record.shared == self.shared(&record.project);
        if !ours {
            return Err(BadPlace::NotOurWorktree);
        }
        Ok(Worktree { thread, record })
    }
}

/// Makes `folder` with access for the user alone.
///
/// # Errors
/// The error of the file system.
pub fn private_folder(folder: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(folder)
}

/// A path with only normal parts: no `..`, no `.`.
pub(super) fn plain(path: &Path) -> bool {
    path.components().all(|part| {
        matches!(
            part,
            Component::Normal(_) | Component::RootDir | Component::Prefix(_)
        )
    })
}

/// The name of the folder of a worktree, made from the name of a repository with a fixed
/// alphabet (letters, digits, `-`, `_`, `.`), never empty, never starting with `.` or `-`.
#[must_use]
pub fn folder_name(repository: &str) -> String {
    let mut name: String = repository
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .take(64)
        .collect();
    while name.starts_with(['.', '-']) {
        name.remove(0);
    }
    if name.is_empty() {
        "repository".to_owned()
    } else {
        name
    }
}

/// The branch the runner makes for a thread: the prefix and the first eight hex digits of the
/// ID of the thread. Nothing of a prompt is in it (THR-050).
#[must_use]
pub fn thread_branch(thread: Uuid) -> String {
    let hex = thread.as_simple().to_string();
    format!("{BRANCH_PREFIX}{}", &hex[..8])
}

/// A branch name that is safe as an argument: the rules of `git check-ref-format` that matter
/// here, and no start with `-`, so that it is never read as an option.
///
/// # Errors
/// [`BadPlace::BadName`] when the name breaks a rule.
pub fn branch_name(name: &str) -> Result<&str, BadPlace> {
    let allowed = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/');
    let good = !name.is_empty()
        && name.len() <= MOST
        && name.chars().all(allowed)
        && !name.starts_with(['-', '/', '.'])
        && !name.ends_with(['/', '.'])
        && !name.contains("..")
        && !name.contains("//")
        && !name.contains("/.")
        && !name
            .split('/')
            .any(|part| part.to_ascii_lowercase().ends_with(".lock"))
        && name != "HEAD";
    if good {
        Ok(name)
    } else {
        Err(BadPlace::BadName)
    }
}

/// A path inside a repository that is safe as a pathspec: relative, no `..`, no NUL, no start
/// with `-` or `:` (the magic of a pathspec), and nothing below `.git`.
///
/// # Errors
/// [`BadPlace::Escapes`] when the path breaks a rule.
pub fn inside(path: &str) -> Result<&str, BadPlace> {
    let parts = Path::new(path);
    let good = !path.is_empty()
        && path.len() <= 4 * MOST
        && !path.contains('\0')
        && !path.starts_with(['-', ':'])
        && parts.is_relative()
        && parts
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
        && !parts
            .components()
            .any(|part| part.as_os_str().eq_ignore_ascii_case(".git"));
    if good {
        Ok(path)
    } else {
        Err(BadPlace::Escapes)
    }
}

#[cfg(test)]
mod tests;
