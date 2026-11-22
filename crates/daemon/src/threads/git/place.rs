//! Where the worktrees of the threads are, and the checks of each path and name before `git`
//! gets it (THR-041, THR-050): a name from a fixed alphabet, a path with no `..`, and a real
//! path that stays below the folder of the worktrees.
//!
//! The checks of text are pure; [`Home::worktree`] reads the file system.

use std::path::{Component, Path, PathBuf};

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
    /// The real folder of the worktree.
    pub folder: PathBuf,
    /// The Git folder of the worktree: `<common>/worktrees/<name>`.
    pub git_dir: PathBuf,
    /// The Git folder of the repository of the person.
    pub common_dir: PathBuf,
    /// The branches the runner made for this thread: the only ones a commit or a push may move.
    pub branches: Vec<String>,
    /// The settings and the hooks of the repository as the person last accepted them
    /// (`trust`): first as they were when the worktree was made, before an agent ran.
    #[serde(default)]
    pub trusted: Option<std::collections::BTreeMap<String, String>>,
    /// What the work folder was when it was made; `None` where the system has no such number.
    #[serde(default)]
    pub folder_id: Option<super::run::folder::FolderId>,
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

    fn record_file(&self, thread: Uuid, repository: &str) -> Result<PathBuf, BadPlace> {
        self.planned(thread, repository)?;
        let thread = thread.as_hyphenated().to_string();
        Ok(self.records.join(thread).join(format!("{repository}.json")))
    }

    /// Writes the record of a worktree, in a folder and a file for the user alone.
    ///
    /// # Errors
    /// A [`BadPlace`] for a bad name, or when the file cannot be written.
    pub fn keep(&self, thread: Uuid, repository: &str, record: &Record) -> Result<(), BadPlace> {
        let file = self.record_file(thread, repository)?;
        let folder = file.parent().ok_or(BadPlace::Escapes)?;
        private_folder(folder).map_err(|_| BadPlace::NotOurWorktree)?;
        let text = serde_json::to_vec_pretty(record).map_err(|_| BadPlace::NotOurWorktree)?;
        std::fs::write(&file, text).map_err(|_| BadPlace::NotOurWorktree)
    }

    /// Removes the record of a worktree that was removed.
    pub fn forget(&self, thread: Uuid, repository: &str) {
        if let Ok(file) = self.record_file(thread, repository) {
            // A record that is gone already is the wanted state.
            let _ = std::fs::remove_file(&file);
            let _ = file.parent().map(std::fs::remove_dir);
        }
    }

    /// The record of the worktree of `repository` of `thread`, as the runner wrote it.
    ///
    /// # Errors
    /// [`BadPlace::NotOurWorktree`] when the runner made no such worktree.
    pub fn recorded(&self, thread: Uuid, repository: &str) -> Result<Record, BadPlace> {
        let file = self.record_file(thread, repository)?;
        let text = std::fs::read(file).map_err(|_| BadPlace::NotOurWorktree)?;
        serde_json::from_slice(&text).map_err(|_| BadPlace::NotOurWorktree)
    }

    /// The worktree of `repository` of `thread`, after each check: the runner has a record of
    /// it; the real path (each symlink followed) is the folder of the record and is below the
    /// real home; its `.git` is a file, as in a linked worktree; and its Git folder is below
    /// the `worktrees` folder of the repository it was made from.
    ///
    /// # Errors
    /// A [`BadPlace`] when a check fails.
    pub fn worktree(&self, thread: Uuid, repository: &str) -> Result<Worktree, BadPlace> {
        let record = self.recorded(thread, repository)?;
        let planned = self.planned(thread, repository)?;
        let home = std::fs::canonicalize(&self.folder).map_err(|_| BadPlace::NotOurWorktree)?;
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
fn plain(path: &Path) -> bool {
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
mod tests {
    use super::*;

    const THREAD: Uuid = Uuid::from_u128(0x1234_5678_9abc_def0_1234_5678_9abc_def0);

    #[test]
    fn the_branch_of_a_thread_has_only_the_prefix_and_hex_digits() {
        assert_eq!(thread_branch(THREAD), "anima/12345678");
    }

    #[test]
    fn a_folder_name_keeps_the_fixed_alphabet_and_never_hides() {
        assert_eq!(folder_name("anima.ing"), "anima.ing");
        assert_eq!(folder_name("../..//etc"), "etc");
        assert_eq!(folder_name("my repo; rm -rf"), "my-repo--rm--rf");
        assert_eq!(folder_name("..."), "repository");
    }

    #[test]
    fn a_branch_name_that_git_could_read_as_an_option_or_a_range_is_refused() {
        for bad in [
            "",
            "-f",
            "--upload-pack=x",
            "a..b",
            "a b",
            "a:b",
            "a~1",
            "a^",
            "HEAD",
            "/a",
            "a/",
            "a//b",
            "a/.b",
            "refs/x.lock",
            "x@{1}",
        ] {
            assert_eq!(branch_name(bad), Err(BadPlace::BadName), "{bad}");
        }
        assert_eq!(
            branch_name("feature/git-stage_2.0"),
            Ok("feature/git-stage_2.0")
        );
    }

    #[test]
    fn a_path_that_leaves_the_repository_is_refused() {
        for bad in [
            "",
            "../x",
            "a/../../x",
            "/etc/passwd",
            "-rf",
            ":(top)x",
            ".git/config",
            "a/.GIT/hooks/x",
            "a\0b",
            "./a",
        ] {
            assert_eq!(inside(bad), Err(BadPlace::Escapes), "{bad:?}");
        }
        assert_eq!(inside("src/lib.rs"), Ok("src/lib.rs"));
        assert_eq!(inside("a file.txt"), Ok("a file.txt"));
    }

    #[test]
    fn a_repository_name_with_a_path_in_it_is_not_a_place() {
        let home =
            Home::at(Path::new("/tmp/worktrees"), Path::new("/tmp/records")).expect("a home");
        assert_eq!(home.planned(THREAD, "../other"), Err(BadPlace::BadName));
        assert_eq!(home.planned(THREAD, "a/b"), Err(BadPlace::BadName));
        let records = Path::new("/tmp/records");
        assert!(Home::at(Path::new("relative"), records).is_err());
        assert!(Home::at(Path::new("/tmp/../etc"), records).is_err());
        assert!(Home::at(Path::new("/tmp/w"), Path::new("/tmp/w/records")).is_err());
    }
}
