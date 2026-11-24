//! The durable directory and retry record for a worktree moved out of use.

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use agent_threads::git::GitRefusal;
use uuid::Uuid;

use super::super::place::{Home, Record};
use super::super::project::{Layout, linked};
use super::super::run::folder::FolderId;

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod movement;

const RETAINED: &str = ".anima-retired";

/// A journal is written before moving the directory. Its ID also names the private directory.
#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct Retirement {
    id: Uuid,
    original: FolderId,
    kept: String,
}

impl Retirement {
    pub(super) fn existing(record: &Record) -> Result<Option<Self>, GitRefusal> {
        match std::fs::read(journal(record)) {
            Ok(text) => {
                let existing: Self = serde_json::from_slice(&text).map_err(failed)?;
                if Some(existing.original) == record.folder_id {
                    return Ok(Some(existing));
                }
                // The old active record was already released. Its directory stays recoverable.
                std::fs::remove_file(journal(record)).map_err(failed)?;
                Ok(None)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(failed(error)),
        }
    }

    pub(super) fn prepare(record: &Record, kept: String) -> Result<Self, GitRefusal> {
        let original = record.folder_id.ok_or(GitRefusal::NoWorktree)?;
        let retirement = Self {
            id: Uuid::now_v7(),
            original,
            kept,
        };
        let text = serde_json::to_vec(&retirement).map_err(failed)?;
        let path = journal(record);
        let temporary = path.with_extension(format!("{}.tmp", retirement.id));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(failed)?;
        let written = file.write_all(&text).and_then(|()| file.sync_all());
        drop(file);
        if let Err(error) = written.and_then(|()| std::fs::rename(&temporary, &path)) {
            let _ = std::fs::remove_file(temporary);
            return Err(failed(error));
        }
        Ok(retirement)
    }

    pub(super) fn finish(
        self,
        home: &Home,
        (thread, repository): (Uuid, &str),
        record: &Record,
    ) -> Result<(String, PathBuf), GitRefusal> {
        let container = self.container(record, (thread, repository));
        let recovery = container.join("worktree");
        let layout = Layout {
            git_dir: record.git_dir.clone(),
            common_dir: record.common_dir.clone(),
        };
        if FolderId::of(&recovery) != Some(self.original) {
            linked::protect(&layout)?;
            self.move_tree(record, &container).map_err(|error| {
                if FolderId::of(&recovery).is_some() {
                    recovery_failed(&recovery, error)
                } else {
                    failed(error)
                }
            })?;
        }
        linked::point_at_recovery(&layout, &recovery)
            .map_err(|error| recovery_failed(&recovery, error))?;
        // The engine's old Git directory must vacate the active name too. Keep it beside
        // the journal on its own filesystem, independently of the retained worktree.
        let held = record
            .own
            .with_file_name(format!("retired-{}.git", self.id));
        if record.own.exists() {
            match held.symlink_metadata() {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(recovery_failed(&recovery, error)),
                Ok(_) => {
                    return Err(recovery_failed(
                        &recovery,
                        "the Git recovery name is occupied",
                    ));
                }
            }
            std::fs::rename(&record.own, held)
                .map_err(|error| recovery_failed(&recovery, error))?;
        }
        home.release(thread, repository)
            .map_err(|error| recovery_failed(&recovery, error))?;
        // A stale journal is identified by its folder identity if a new worktree reuses the name.
        let _ = std::fs::remove_file(journal(record));
        Ok((self.kept, recovery))
    }

    fn container(&self, record: &Record, (thread, repository): (Uuid, &str)) -> PathBuf {
        let name = format!("{}-{repository}-{}", thread.as_simple(), self.id);
        record.home.join(RETAINED).join(name)
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    fn move_tree(&self, record: &Record, container: &Path) -> io::Result<()> {
        movement::retire(record, container, self.original)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    fn move_tree(&self, _record: &Record, _container: &Path) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests;

fn journal(record: &Record) -> PathBuf {
    record.own.with_extension("retiring")
}

fn failed(error: impl std::fmt::Display) -> GitRefusal {
    GitRefusal::Failed {
        said: format!("Could not retire the worktree: {error}"),
    }
}

fn recovery_failed(recovery: &Path, error: impl std::fmt::Display) -> GitRefusal {
    GitRefusal::Failed {
        said: format!(
            "Worktree recovery is at {}. Could not finish removal: {error}. Retry removal to finish.",
            recovery.display().to_string().escape_debug()
        ),
    }
}
