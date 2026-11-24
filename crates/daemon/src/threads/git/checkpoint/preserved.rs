//! The source paths that the removal snapshot preserves, including files saved under aliases.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::super::place::Worktree;
use super::super::run::{Git, GitError, Run};
use super::super::worktree::files::{self, Root};

pub struct Preserved(HashSet<PathBuf>);

impl Preserved {
    pub(super) async fn from_index(
        git: &Git,
        worktree: &Worktree,
        aliases: Vec<(PathBuf, PathBuf)>,
    ) -> Result<Self, GitError> {
        let run = Run::new(worktree.folder(), ["ls-files", "--stage", "-z"]);
        let indexed = git.output(run).await?;
        let mut paths: HashSet<PathBuf> = indexed
            .split(|byte| *byte == 0)
            .filter_map(|entry| {
                let mut parts = entry.splitn(2, |byte| *byte == b'\t');
                let metadata = parts.next()?;
                let path = parts.next()?;
                // A submodule entry preserves a commit ID, never a replacement file
                // or link that happens to occupy the same path in the worktree.
                (!metadata.starts_with(b"160000 ")).then(|| files::path(path))
            })
            .collect();
        for (source, saved_at) in aliases {
            if paths.remove(&saved_at) {
                paths.insert(source);
            }
        }
        Ok(Self(paths))
    }

    /// Refuses removal if a file or link would go without a corresponding saved path.
    pub(in crate::threads::git) fn check(&self, folder: &Path) -> Result<(), GitError> {
        let found = Root::open(folder)
            .and_then(|root| root.files_below(Path::new("")))
            .map_err(|failed| GitError::Failed {
                code: None,
                said: format!("Could not account for the worktree files: {failed}"),
            })?;
        if let Some(path) = found.iter().find(|path| !self.0.contains(*path)) {
            return Err(GitError::Failed {
                code: None,
                said: format!(
                    "The worktree contains a file that Git would not preserve: {}. Move it outside the worktree and retry.",
                    files::shown(path)
                ),
            });
        }
        Ok(())
    }
}
