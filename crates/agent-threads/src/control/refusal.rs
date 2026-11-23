//! Why the runner did not do a call: the refusals of the local socket (TH12).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::Limits;
#[cfg(doc)]
use super::{MOST_QUEUED, attach};

/// Why the runner did not do a call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum Refusal {
    UnknownThread,
    /// The folder is not an absolute folder of this computer (THR-041).
    FolderNotPermitted,
    /// The program is not on the `PATH` of the login of the person.
    ProgramNotFound,
    /// The program did not answer `--version` in time: `said` names it, its path and what to
    /// try, in words for the person; `answered` holds the other copies that answered, which a
    /// start can name (TH97).
    ProgramDoesNotAnswer {
        said: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        answered: Vec<PathBuf>,
    },
    /// A value that becomes an argument is not of its alphabet (THR-040).
    BadValue,
    /// Creating or registering a project failed (TH60A).
    CannotCreateProject {
        said: String,
    },
    /// The experimental provider route cannot safely start (TH118).
    ProviderAddress {
        said: String,
    },
    /// The request is answered, or its process ended (THR-030).
    RequestGone,
    /// The answer does not fit the request.
    AnswerDoesNotFit,
    /// The thread is not running.
    NotRunning,
    /// The holder did not take the command.
    Holder,
    /// The holder is older than the daemon and speaks a different protocol: `said` tells when
    /// it is replaced, in words for the person (TH98).
    HolderOlder {
        said: String,
    },
    /// The call came from a process of an agent: only the person answers a request, raises the
    /// limit or starts a thread.
    FromAnAgent,
    /// The thread runs: it is removed from the list only after it stopped (TH46).
    Running,
    /// A start would pass the soft limit: the person must answer first (THR-114).
    OverLimit {
        limits: Limits,
    },
    /// The queue of the thread holds [`MOST_QUEUED`] prompts (TH109).
    QueueFull,
    /// The program of the thread cannot change that after its start: `said` tells what and
    /// why, in words for the person (TH110).
    CannotChange {
        said: String,
    },
    /// A file is over a limit of [`attach`]: `said` names the file and the limit (TH111).
    TooLarge {
        said: String,
    },
    /// The runner cannot take or send that file: `said` names the file and why (TH111).
    CannotAttach {
        said: String,
    },
    /// A folder that the sandbox of a new thread could write is shared with a different running
    /// thread, or holds the records of the Git engine: `said` names it and why (THR-050).
    FolderShared {
        said: String,
    },
    /// The worktree of a new thread was not made: `why` is the answer of the Git engine
    /// (TC7, TH116).
    Worktree {
        why: crate::git::GitRefusal,
    },
}
