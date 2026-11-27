//! Revalidate the records boundary immediately before Git opens a private repository.

use std::path::Path;

use super::super::place::records::Directory;
use super::{Git, GitError, Run};

impl Git {
    pub(in super::super) fn with_records(mut self, root: &Path) -> Self {
        self.records = Some(root.to_owned());
        self
    }

    pub(super) fn check_records(&self, run: &Run<'_>) -> Result<(), GitError> {
        let Some(root) = &self.records else {
            return Ok(());
        };
        let entries = run
            .env
            .iter()
            .chain(&self.bound)
            .filter(|(name, _)| *name == "GIT_DIR")
            .map(|(_, path)| Path::new(path))
            .filter(|path| path.starts_with(root));
        Directory::open(root, root, false).map_err(GitError::Files)?;
        if run.dir.starts_with(root) && run.dir != root {
            Directory::open(root, run.dir, false).map_err(GitError::Files)?;
        }
        for git_dir in entries {
            match Directory::open(root, git_dir, false) {
                Ok(_) => {}
                // Git init is allowed to create a Git directory in a checked parent.
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound
                        && run.args.first().is_some_and(|arg| arg == "init") =>
                {
                    let parent = git_dir.parent().ok_or(GitError::NotFound)?;
                    Directory::open(root, parent, false).map_err(GitError::Files)?;
                }
                Err(error) => return Err(GitError::Files(error)),
            }
        }
        Ok(())
    }
}
