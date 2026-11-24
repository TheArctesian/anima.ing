//! The record of the runner (THR-030, THR-047): the threads it started, their open requests and
//! the soft limits, in a file of mode 0600, so that a runner that starts again finds them and
//! resumes only its own threads.

use std::io::Write as _;
use std::path::Path;

use agent_threads::control::{Change, Limits, Listing, Project, Queued, RunState, Workspace};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::thread::{Saved, Step, Thread};
use super::{Runner, RunnerError, calls};

/// The lock file of the runner in the folder of the threads: one runner for one folder.
const LOCK: &str = "runner.lock";

/// The name of the record in the folder of the threads.
pub const FILE: &str = "threads.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct Record {
    pub limits: Limits,
    pub threads: Vec<Saved>,
    /// How the person lists each thread (TH46); a record of before has none.
    #[serde(default)]
    pub listings: Vec<Listing>,
    /// The projects of this computer (TH107); a record of before has none.
    #[serde(default)]
    pub projects: Vec<Project>,
}

impl Default for Record {
    fn default() -> Self {
        Self {
            limits: Limits::FIRST,
            threads: Vec::new(),
            listings: Vec::new(),
            projects: Vec::new(),
        }
    }
}

/// The record in `dir`; an empty one when there is none. A record that does not read is a
/// fault: the runner does not start over it, so that no thread is lost from view.
///
/// # Errors
/// The I/O error, or [`std::io::ErrorKind::InvalidData`] for a record that does not read.
pub fn read(dir: &Path) -> std::io::Result<Record> {
    match std::fs::read(dir.join(FILE)) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Record::default()),
        Err(error) => Err(error),
    }
}

/// Writes the record: to a new file of mode 0600 first, then in place of the old one.
///
/// # Errors
/// The I/O error of the file.
pub fn write(dir: &Path, record: &Record) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let text = serde_json::to_vec(record).map_err(std::io::Error::other)?;
    let new = dir.join(format!("{FILE}.new"));
    // A file left by an earlier write may have a different mode: a new one is made 0600.
    match std::fs::remove_file(&new) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error),
        _ => {}
    }
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&new)?;
    file.write_all(&text)?;
    file.sync_all()?;
    std::fs::rename(new, dir.join(FILE))?;
    // The new name is on the disk only when the folder is too. A disk that cannot sync a
    // folder (some network and FAT disks) keeps the record as its files are kept.
    match std::fs::File::open(dir)?.sync_all() {
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::Unsupported | std::io::ErrorKind::InvalidInput
            ) =>
        {
            Ok(())
        }
        synced => synced,
    }
}

/// Makes the folder of the threads, and checks that it is a private folder of this user.
pub(super) fn private_folder(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt as _;
    if !dir.exists() {
        std::fs::DirBuilder::new()
            .mode(0o700)
            .recursive(true)
            .create(dir)?;
    }
    if crate::config::private_file::folder_private(dir)? {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "the folder of the threads is not a private folder of this user",
        ))
    }
}

/// Takes the lock of the folder: a second runner on the same folder ends at once, so that two
/// runners never stop the programs of each other (THR-113).
pub(super) fn lock(dir: &Path) -> Result<std::fs::File, RunnerError> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(dir.join(LOCK))?;
    rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive)
        .map_err(|_| RunnerError::Busy)?;
    Ok(file)
}

/// The longest wait for a read of where a thread works.
const WORKSPACE_READ: std::time::Duration = std::time::Duration::from_secs(2);

impl Runner {
    /// The threads of the record of a run before, each with its updates numbered after the
    /// updates of that run.
    pub(super) fn restore(&mut self, saved: Vec<Saved>) {
        for saved in saved {
            let daemon = self.dir.parent().unwrap_or(&self.dir);
            let codex = calls::codex_of(&saved.row, saved.effort, daemon);
            let mut thread = Thread::restore(saved, codex);
            // A thread of a record from before TH112 learns where it works; the branch is read
            // again at the end of its next turn.
            let folder = thread.row.folder.clone();
            thread
                .row
                .workspace
                .get_or_insert_with(|| Workspace::of(&folder));
            // The microseconds of the clock are more than the updates of any run before.
            thread.number_from(u64::try_from(chrono::Utc::now().timestamp_micros()).unwrap_or(0));
            let id = thread.row.id;
            let steps = thread.settle_answering();
            self.threads.insert(id, thread);
            // A thread of a record from before the mark gets it now (TH61).
            self.mark_started(id);
            for step in steps {
                if let Step::Change(change) = step {
                    self.publish(id, change);
                }
            }
        }
        // The launcher knows the restored threads also when none changed.
        self.rights_now();
    }

    /// Reads where a thread works again; the clients get the row when it changed (TH112). The
    /// read is off the loop of the runner and has a time limit, as the folder can be on a
    /// network that does not answer; then the row keeps what it had.
    pub(super) async fn look_at_workspace(&mut self, id: Uuid) {
        let Some(folder) = self
            .threads
            .get(&id)
            .map(|thread| thread.row.folder.clone())
        else {
            return;
        };
        let read = tokio::task::spawn_blocking(move || Workspace::of(&folder));
        let Ok(Ok(now)) = tokio::time::timeout(WORKSPACE_READ, read).await else {
            tracing::warn!(%id, "the workspace of a thread did not read in time");
            return;
        };
        let Some(thread) = self.threads.get_mut(&id) else {
            return;
        };
        if thread.row.workspace.as_ref() == Some(&now) {
            return;
        }
        thread.row.workspace = Some(now);
        let change = Change::State {
            state: thread.row.state,
            row: Box::new(thread.row.clone()),
        };
        self.publish(id, change);
    }
}

impl Runner {
    /// Writes the record, after it lets go of the oldest stopped threads.
    pub(super) fn save(&mut self) {
        // A record that was not written is logged; the threads go on.
        let _ = self.written();
    }

    /// Writes the record, after it lets go of the oldest stopped threads; whether it was
    /// written.
    pub(super) fn written(&mut self) -> bool {
        let mut stopped: Vec<(chrono::DateTime<chrono::Utc>, Uuid)> = self
            .threads
            .values()
            .filter(|thread| thread.row.state == RunState::Stopped)
            .map(|thread| (thread.row.started, thread.row.id))
            .collect();
        stopped.sort();
        let over = stopped.len().saturating_sub(super::STOPPED_KEPT);
        for (_, id) in stopped.into_iter().take(over) {
            self.threads.remove(&id);
            self.read_up_to.remove(&id);
            // No prompt can go to it again: its files go with it (TH111).
            self.attached.forget(id);
        }
        let record = Record {
            limits: self.limits,
            threads: self.threads.values().map(Thread::saved).collect(),
            listings: self.listings.values().cloned().collect(),
            projects: self.projects.clone(),
        };
        match write(&self.dir, &record) {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(%error, "the record of the runner was not written");
                false
            }
        }
    }
}

impl Runner {
    /// Save the recovery identity of a Grok prompt and a Codex profile departure before
    /// dispatch. If the record fails, the prompt stays here (THR-041).
    pub(super) fn held_back(&mut self, id: Uuid, line: &str) -> bool {
        let must_save = self
            .threads
            .get(&id)
            .is_some_and(|thread| thread.leaves_with(line) || thread.grok_prompt(line));
        if !must_save || self.written() {
            return false;
        }
        let ended = self
            .threads
            .get_mut(&id)
            .map(|thread| thread.held_back(line));
        for step in ended.unwrap_or_default() {
            self.own_step(id, step, None);
        }
        true
    }

    /// The prompt of the queue whose line was held back waits again, first; whether it does.
    pub(super) fn waits_first(&mut self, id: Uuid, queued: &Queued) -> bool {
        let back = self
            .threads
            .get_mut(&id)
            .and_then(|thread| thread.put_back(queued.id, queued.text.clone()));
        let waits = back.is_some();
        if let Some(back) = back {
            self.own_step(id, back, None);
        }
        waits
    }
}
