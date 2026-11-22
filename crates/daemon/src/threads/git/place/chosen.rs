//! The home of the worktrees: `worktrees` in the home of Anima, or a folder that the person
//! chose in the settings. A chosen folder is input that is not trusted: each link is resolved
//! first, and a folder inside a repository, inside a folder of Anima, or inside a folder that
//! an agent can write is refused. A worktree that exists stays where it was made
//! ([`super::Record::home`]); nothing is moved when the setting changes.
//!
//! I/O: reads the file system to resolve the path and to look for a repository above it.

use std::path::{Path, PathBuf};

use super::{BadPlace, Home};

/// The name of the home of the worktrees in the home of Anima.
pub const STANDARD: &str = "worktrees";

impl Home {
    /// The home that is used when the person chose none: `<home of Anima>/worktrees`.
    ///
    /// # Errors
    /// As [`Home::at`].
    pub fn standard(anima: &Path, records: &Path) -> Result<Self, BadPlace> {
        Self::at(&anima.join(STANDARD), records)
    }

    /// The home at the folder that the person chose. `kept_out` are the folders that it must
    /// not be in and must not hold: the home of Anima, the home of the daemon, and each folder
    /// that the sandbox of an agent can write. The answer holds the real path.
    ///
    /// # Errors
    /// [`BadPlace::Escapes`] for a path that is relative, has `..`, has a link that leads
    /// nowhere, or is in or around the records; [`BadPlace::InARepository`];
    /// [`BadPlace::KeptOut`].
    pub fn chosen(folder: &Path, records: &Path, kept_out: &[PathBuf]) -> Result<Self, BadPlace> {
        if !folder.is_absolute() || !super::plain(folder) {
            return Err(BadPlace::Escapes);
        }
        let folder = real(folder).ok_or(BadPlace::Escapes)?;
        let in_a_repository = folder
            .ancestors()
            .any(|above| above.join(".git").symlink_metadata().is_ok());
        if in_a_repository {
            return Err(BadPlace::InARepository);
        }
        for root in kept_out {
            // A folder to keep out of that cannot be resolved keeps each folder out.
            let root = real(root).ok_or(BadPlace::KeptOut)?;
            if folder.starts_with(&root) || root.starts_with(&folder) {
                return Err(BadPlace::KeptOut);
            }
        }
        Self::at(&folder, records)
    }
}

/// The real path of `path`: each link of the part that exists is followed, and the part that
/// does not exist yet is added as it is. `None` when a link on the way leads nowhere.
fn real(path: &Path) -> Option<PathBuf> {
    let mut missing = Vec::new();
    let mut at = path;
    loop {
        if let Ok(real) = std::fs::canonicalize(at) {
            return Some(
                missing
                    .iter()
                    .rev()
                    .fold(real, |path, part| path.join(part)),
            );
        }
        if at.symlink_metadata().is_ok() {
            return None;
        }
        missing.push(at.file_name()?);
        at = at.parent()?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> (tempfile::TempDir, PathBuf) {
        let keep = tempfile::tempdir().expect("a folder");
        let root = std::fs::canonicalize(keep.path()).expect("its real path");
        (keep, root)
    }

    #[test]
    fn the_standard_home_is_in_the_home_of_anima_and_a_chosen_one_is_not() {
        let (_keep, root) = root();
        let (anima, records) = (root.join("anima"), root.join("anima/daemon/records"));
        let home = Home::standard(&anima, &records).expect("the standard home");
        assert_eq!(home.folder(), anima.join("worktrees"));
        let kept_out = [anima.clone()];
        let inside = Home::chosen(&anima.join("mine"), &records, &kept_out);
        assert_eq!(inside, Err(BadPlace::KeptOut));
        // A folder that holds the home of Anima would hold its records too.
        assert_eq!(
            Home::chosen(&root, &records, &kept_out),
            Err(BadPlace::KeptOut)
        );
        let free = Home::chosen(&root.join("code/trees"), &records, &kept_out);
        assert_eq!(free.expect("a home").folder(), root.join("code/trees"));
    }

    #[test]
    fn a_folder_in_a_repository_or_named_with_dots_is_refused() {
        let (_keep, root) = root();
        let records = root.join("records");
        std::fs::create_dir_all(root.join("project/.git")).expect("a repository");
        let inside = Home::chosen(&root.join("project/sub/trees"), &records, &[]);
        assert_eq!(inside, Err(BadPlace::InARepository));
        let dots = root.join("free/../project/trees");
        assert_eq!(Home::chosen(&dots, &records, &[]), Err(BadPlace::Escapes));
        let relative = Path::new("trees");
        assert_eq!(
            Home::chosen(relative, &records, &[]),
            Err(BadPlace::Escapes)
        );
        assert_eq!(
            Home::chosen(&records.join("trees"), &records, &[]),
            Err(BadPlace::Escapes)
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_resolved_before_each_check() {
        let (_keep, root) = root();
        let records = root.join("records");
        std::fs::create_dir_all(root.join("project/.git")).expect("a repository");
        std::fs::create_dir_all(root.join("shared")).expect("a folder");
        let link = |to: &str, name: &str| {
            std::os::unix::fs::symlink(root.join(to), root.join(name)).expect("a link");
            root.join(name)
        };
        let into_repository = link("project", "innocent");
        let found = Home::chosen(&into_repository.join("trees"), &records, &[]);
        assert_eq!(found, Err(BadPlace::InARepository));
        let into_shared = link("shared", "plain");
        let kept_out = [root.join("shared")];
        let found = Home::chosen(&into_shared.join("trees"), &records, &kept_out);
        assert_eq!(found, Err(BadPlace::KeptOut));
        let nowhere = link("gone", "dangling");
        let found = Home::chosen(&nowhere.join("trees"), &records, &[]);
        assert_eq!(found, Err(BadPlace::Escapes));
    }
}
