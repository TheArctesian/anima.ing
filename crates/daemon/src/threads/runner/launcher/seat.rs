//! A thread as the launcher knows it: its folder, its confinement, its `TMPDIR` and the real
//! paths of the folders that its sandbox lets its agent write (THR-050).
//!
//! The runner checks a new seat against the seats of the other running threads before its
//! agent starts ([`Seat::shared`]).
//!
//! I/O: resolves the paths of the folders.

use std::path::{Path, PathBuf};
use std::sync::PoisonError;

use agent_threads::drive::Mode;
use uuid::Uuid;

use super::super::Runner;
use super::read::Confinement;
use crate::threads::git::place::records::held_by;

/// A thread for the launcher: its folder, how its agent is confined, its `TMPDIR` and the
/// folders that its sandbox lets it write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seat {
    pub folder: PathBuf,
    pub confinement: Confinement,
    /// The private `TMPDIR` of the thread ([`super::super::start::private_tmp`]).
    pub tmp: PathBuf,
    /// The real paths of the folders that the sandbox of its agent can write: its folder, its
    /// `TMPDIR` and `/tmp`, resolved once, when the seat is made. None in Full access.
    pub roots: Vec<PathBuf>,
}

impl Seat {
    /// The seat of a thread in `folder` with `tmp`. Its folder and `/tmp` are resolved here, one
    /// time; `tmp` is kept as [`made`] gave it, the path that the agent gets, so a link put there
    /// later does not move the root. A root that does not resolve stays as it is named:
    /// [`held_by_any`] counts it as one that holds.
    ///
    /// [`made`]: super::super::start::private_tmp::made
    /// [`held_by_any`]: crate::threads::git::place::records::held_by_any
    #[must_use]
    pub fn new(folder: PathBuf, confinement: Confinement, tmp: PathBuf) -> Self {
        let real = |root: &Path| std::fs::canonicalize(root).unwrap_or_else(|_| root.to_owned());
        let roots = if confinement.mode == Mode::FullAccess {
            Vec::new()
        } else {
            vec![real(&folder), tmp.clone(), shared_tmp()]
        };
        Self {
            folder,
            confinement,
            tmp,
            roots,
        }
    }
}

impl Seat {
    /// Why the sandbox of this new thread must not start, or `None`. No root of it but `/tmp`
    /// may lie inside or hold a root of a different running sandboxed thread, in either
    /// direction: that agent could put a link on its path between the check and the start of the
    /// sandbox, or write the `TMPDIR` of the other. Two threads in the very same folder are
    /// permitted. An extra root, one besides its own folder and `/tmp`, may not hold `records`:
    /// what an agent puts there stays after its thread ends (THR-050). A thread in Full access is
    /// not checked (operator's ruling of 2026-11-23).
    #[must_use]
    pub fn shared<'a>(
        &self,
        others: impl IntoIterator<Item = &'a Self>,
        records: Option<&Path>,
    ) -> Option<String> {
        let tmp = shared_tmp();
        let extra: Vec<_> = self
            .roots
            .iter()
            .skip(1)
            .filter(|root| **root != tmp)
            .collect();
        let holds = |root: &&&PathBuf| records.is_some_and(|records| held_by(records, root));
        if let Some(root) = extra.iter().find(holds) {
            return Some(format!(
                "The folder {} that the new thread could write holds the records of the Git engine.",
                root.display()
            ));
        }
        // Each root but `/tmp`, with whether it is the folder of its thread.
        let own = |seat: &'a Self| {
            let tmp = tmp.clone();
            seat.roots
                .iter()
                .enumerate()
                .filter(move |(_, root)| **root != tmp)
                .map(|(at, root)| (at == 0, root))
        };
        let theirs: Vec<_> = others.into_iter().flat_map(own).collect();
        let mut mine = self
            .roots
            .iter()
            .enumerate()
            .filter(|(_, root)| **root != tmp)
            .map(|(at, root)| (at == 0, root));
        mine.find_map(|(folder, root)| {
            let (_, near) = theirs.iter().find(|(their_folder, their)| {
                let one_folder = folder && *their_folder && root == *their;
                !one_folder && (root.starts_with(their) || their.starts_with(root))
            })?;
            Some(format!(
                "The folder {} that the new thread could write is in or holds the folder {} that a different running thread can write.",
                root.display(),
                near.display()
            ))
        })
    }
}

impl Runner {
    /// Why the sandbox of the new thread `id` must not start ([`Seat::shared`]): checked with
    /// the seats as `Runner::rights_now` wrote them, against each other running thread.
    pub(in crate::threads::runner) fn shared_root(&self, id: Uuid) -> Option<String> {
        let rights = self.rights.lock().unwrap_or_else(PoisonError::into_inner);
        let seat = rights.threads.get(&id)?;
        let others = rights
            .threads
            .iter()
            .filter(|(other, _)| **other != id && !rights.coming.contains_key(other))
            .map(|(_, seat)| seat);
        seat.shared(others, rights.records.as_deref())
    }
}

/// `/tmp` as each seat resolves it: the one root that each sandboxed thread shares.
fn shared_tmp() -> PathBuf {
    let tmp = Path::new("/tmp");
    std::fs::canonicalize(tmp).unwrap_or_else(|_| tmp.to_owned())
}
