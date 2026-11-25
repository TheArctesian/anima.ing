//! The durable directory and retry record for a worktree moved out of use.

use std::io;
use std::path::{Path, PathBuf};

use agent_threads::git::{GitOutcome, GitRefusal};
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
    pub(super) fn existing(home: &Home, record: &Record) -> Result<Option<Self>, GitRefusal> {
        match home.read_state(&journal(record)) {
            Ok(Some(text)) => {
                let existing: Self = serde_json::from_slice(&text).map_err(failed)?;
                if Some(existing.original) == record.folder_id {
                    return Ok(Some(existing));
                }
                // The old active record was already released. Its directory stays recoverable.
                home.remove_state(&journal(record)).map_err(failed)?;
                Ok(None)
            }
            Ok(None) => Ok(None),
            Err(error) => Err(failed(error)),
        }
    }

    pub(super) fn prepare(home: &Home, record: &Record, kept: String) -> Result<Self, GitRefusal> {
        let original = record.folder_id.ok_or(GitRefusal::NoWorktree)?;
        let retirement = Self {
            id: Uuid::now_v7(),
            original,
            kept,
        };
        write(home, &journal(record), &retirement).map_err(failed)?;
        Ok(retirement)
    }

    pub(super) fn finish(
        self,
        home: &Home,
        target: (Uuid, &str),
        record: &Record,
    ) -> Result<GitOutcome, GitRefusal> {
        let container = self.container(record, target);
        let recovery = container.join("worktree");
        let completed = self.complete(home, target, (record, &container));
        let completed = completed.map_err(|error| {
            let location = if verify(&recovery, self.original).is_ok() {
                format!("Worktree recovery is at {}.", shown(&recovery))
            } else if source(record).is_ok_and(|found| found == Some(self.original)) {
                format!("The worktree is still at {}.", shown(&record.folder))
            } else {
                "The worktree location could not be verified.".to_owned()
            };
            GitRefusal::Failed {
                said: format!(
                    "{location} Could not finish removal: {error}. Retry removal to finish."
                ),
            }
        })?;
        // A stale journal is identified by its folder identity if a new worktree reuses the name.
        let _ = home.remove_state(&journal(record));
        Ok(completed)
    }

    fn complete(
        &self,
        home: &Home,
        target: (Uuid, &str),
        (record, container): (&Record, &Path),
    ) -> io::Result<GitOutcome> {
        let recovery = container.join("worktree");
        home.private_git_exists(&record.own)?;
        // Recheck recovery after the original so a move between the probes cannot look absent.
        if destination(record, container)?.is_none()
            && source(record)?.is_none()
            && destination(record, container)?.is_none()
        {
            return Completion::without_copy(record, Some(self.kept.clone())).release(
                home,
                target,
                (&record.own, self.id),
            );
        }
        let layout = Layout {
            git_dir: record.git_dir.clone(),
            common_dir: record.common_dir.clone(),
        };
        linked::protect(&layout).map_err(io::Error::other)?;
        self.move_tree(record, container)?;
        verify(&recovery, self.original)?;
        linked::point_at_recovery(&layout, &recovery).map_err(io::Error::other)?;
        Completion {
            kept: Some(self.kept.clone()),
            recovery: Some(recovery),
            original: record.folder.clone(),
            proof: Some(self.original),
        }
        .release(home, target, (&record.own, self.id))
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

/// The last completed removal remains available after its active registration is released.
#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct Completion {
    kept: Option<String>,
    recovery: Option<PathBuf>,
    original: PathBuf,
    proof: Option<FolderId>,
}

impl Completion {
    pub(super) fn get(
        home: &Home,
        (thread, repository): (Uuid, &str),
    ) -> Result<GitOutcome, GitRefusal> {
        let path = home.own(thread, repository)?.with_extension("removed");
        let text = home
            .read_state(&path)
            .map_err(failed)?
            .ok_or(GitRefusal::NoWorktree)?;
        let completed: Self = serde_json::from_slice(&text).map_err(failed)?;
        if let Some(recovery) = &completed.recovery {
            let proof = completed
                .proof
                .ok_or_else(|| failed("recovery receipt has no folder identity"))?;
            verify(recovery, proof).map_err(failed)?;
        }
        Ok(completed.outcome())
    }

    pub(super) fn orphan(
        home: &Home,
        (thread, repository): (Uuid, &str),
        record: &Record,
    ) -> Result<GitOutcome, GitRefusal> {
        if source(record).map_err(failed)?.is_some()
            && home.private_git_exists(&record.own).map_err(failed)?
        {
            return Err(failed(
                "the original worktree or its records could not be verified",
            ));
        }
        Self::without_copy(record, None)
            .release(home, (thread, repository), (&record.own, Uuid::now_v7()))
            .map_err(failed)
    }

    fn without_copy(record: &Record, kept: Option<String>) -> Self {
        Self {
            kept,
            recovery: None,
            original: record.folder.clone(),
            proof: None,
        }
    }

    fn release(
        self,
        home: &Home,
        (thread, repository): (Uuid, &str),
        (own, id): (&Path, Uuid),
    ) -> io::Result<GitOutcome> {
        let expected = home.own(thread, repository).map_err(io::Error::other)?;
        if expected != own {
            return Err(io::ErrorKind::InvalidData.into());
        }
        archive(home, own, id)?;
        write(home, &expected.with_extension("removed"), &self)?;
        home.release(thread, repository)?;
        Ok(self.outcome())
    }

    fn outcome(self) -> GitOutcome {
        GitOutcome::Removed {
            kept: self.kept,
            recovery: self.recovery,
            original: Some(self.original),
        }
    }
}

fn write(home: &Home, path: &Path, value: &impl serde::Serialize) -> io::Result<()> {
    home.write_state(path, &serde_json::to_vec(value)?)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn archive(home: &Home, source: &Path, id: Uuid) -> io::Result<()> {
    let parent = home.state_parent(source, false)?;
    let name = source.file_name().ok_or(io::ErrorKind::InvalidInput)?;
    let saved = format!("retired-{id}.git");
    parent.archive(name, saved.as_ref())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn archive(_home: &Home, _source: &Path, _id: Uuid) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
use movement::{destination, source};

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn destination(_record: &Record, _container: &Path) -> io::Result<Option<FolderId>> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn source(_record: &Record) -> io::Result<Option<FolderId>> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn verify(recovery: &Path, original: FolderId) -> io::Result<()> {
    let container = recovery.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let home = container
        .parent()
        .and_then(Path::parent)
        .ok_or(io::ErrorKind::InvalidInput)?;
    movement::verify(home, container, original)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn verify(_recovery: &Path, _original: FolderId) -> io::Result<()> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
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

fn shown(path: &Path) -> String {
    path.display().to_string().escape_debug().to_string()
}
