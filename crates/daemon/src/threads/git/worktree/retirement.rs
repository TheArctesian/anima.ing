//! The durable directory and retry record for a worktree moved out of use.

use std::io::{self, Write as _};
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
        write(&journal(record), &retirement).map_err(failed)?;
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
        linked::protect(&layout)?;
        self.move_tree(record, &container)
            .map_err(|error| recovery_failed(&recovery, error))?;
        linked::point_at_recovery(&layout, &recovery)
            .map_err(|error| recovery_failed(&recovery, error))?;
        archive(&record.own, self.id).map_err(|error| recovery_failed(&recovery, error))?;
        verify(&recovery, self.original).map_err(|error| recovery_failed(&recovery, error))?;
        let completion = Completion {
            kept: Some(self.kept.clone()),
            recovery: Some(recovery.clone()),
            original: record.folder.clone(),
            proof: Some(self.original),
        };
        completion
            .store(home, (thread, repository))
            .map_err(|error| recovery_failed(&recovery, error))?;
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
        let text = std::fs::read(path).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                GitRefusal::NoWorktree
            } else {
                failed(error)
            }
        })?;
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
        let completed = Self {
            kept: None,
            recovery: None,
            original: record.folder.clone(),
            proof: None,
        };
        archive(&record.own, Uuid::now_v7()).map_err(failed)?;
        completed
            .store(home, (thread, repository))
            .map_err(failed)?;
        home.release(thread, repository).map_err(failed)?;
        Ok(completed.outcome())
    }

    fn store(&self, home: &Home, (thread, repository): (Uuid, &str)) -> io::Result<()> {
        let own = home.own(thread, repository).map_err(io::Error::other)?;
        write(&own.with_extension("removed"), self)
    }

    fn outcome(self) -> GitOutcome {
        GitOutcome::Removed {
            kept: self.kept,
            recovery: self.recovery,
            original: Some(self.original),
        }
    }
}

fn write(path: &Path, value: &impl serde::Serialize) -> io::Result<()> {
    let parent = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(&mut tmp, value)?;
    tmp.flush()?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|error| error.error)?;
    std::fs::File::open(parent)?.sync_all()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn archive(source: &Path, id: Uuid) -> io::Result<()> {
    let saved = format!("retired-{id}.git");
    movement::archive(source, saved.as_ref())
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn archive(_source: &Path, _id: Uuid) -> io::Result<()> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
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

fn recovery_failed(recovery: &Path, error: impl std::fmt::Display) -> GitRefusal {
    GitRefusal::Failed {
        said: format!(
            "Worktree recovery is at {}. Could not finish removal: {error}. Retry removal to finish.",
            recovery.display().to_string().escape_debug()
        ),
    }
}
