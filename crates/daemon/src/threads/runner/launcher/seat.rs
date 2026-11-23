//! A thread as the launcher knows it: its folder, its confinement, its `TMPDIR` and the real
//! paths of the folders that its sandbox lets its agent write (THR-050).
//!
//! I/O: resolves the paths of the folders.

use std::path::{Path, PathBuf};

use agent_threads::drive::Mode;

use super::read::Confinement;

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
            vec![real(&folder), tmp.clone(), real(Path::new("/tmp"))]
        };
        Self {
            folder,
            confinement,
            tmp,
            roots,
        }
    }
}
