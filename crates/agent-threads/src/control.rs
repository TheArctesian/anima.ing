//! The local socket between a client of this computer (the `anima` command, the desktop shell)
//! and the thread runner of the daemon (TH12, THR-128): one line of JSON for each frame.
//!
//! Pure: the frames and the place of the socket. The runner and the clients do the I/O.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::drive::{Answer, DriveEvent, Effort, Mode, Program, Request, Usage};
use crate::files::FoundPath;

pub mod attach;
mod limits;
mod listing;
mod programs;
mod project;
mod queue;
mod refusal;
mod shelf;
mod workspace;
pub use limits::{Limits, OVER_LIMIT};
pub use listing::{BadChange, BadTitle, Listing, ListingChange, MAX_TITLE};
pub use programs::{AccountRow, ListState, ModelRow, ProgramState};
pub use project::{BadProject, MAX_PROJECTS, Project, ProjectChange, changed as project_changed};
pub use queue::{LastTurn, MOST_QUEUED, MOST_QUEUED_BYTES, Queued};
pub use refusal::Refusal;
pub use shelf::{Activity, MOST_SNOOZE, Marks, SETTLE_AFTER, Shelf};
pub use workspace::{Workspace, WorkspaceKind};

/// The most bytes of one frame of a client.
pub const MAX_FRAME: usize = 4 * 1024 * 1024;

/// The name of the socket file in the folder of the threads of the daemon.
pub const SOCKET: &str = "runner.sock";

/// The folder of the threads of the daemon: `threads` below the folder of the daemon, which is
/// `ANIMA_DAEMON_HOME`, else on macOS `~/Library/Application Support/anima-daemon`, else
/// `$XDG_CONFIG_HOME/anima-daemon` or `~/.config/anima-daemon` (as `Home::locate` of the daemon).
#[must_use]
pub fn folder(env: impl Fn(&str) -> Option<String>, macos: bool) -> Option<PathBuf> {
    let daemon = if let Some(set) = env("ANIMA_DAEMON_HOME") {
        PathBuf::from(set)
    } else if macos {
        Path::new(&env("HOME")?).join("Library/Application Support/anima-daemon")
    } else if let Some(config) = env("XDG_CONFIG_HOME").filter(|dir| !dir.is_empty()) {
        Path::new(&config).join("anima-daemon")
    } else {
        Path::new(&env("HOME")?).join(".config/anima-daemon")
    };
    Some(daemon.join("threads"))
}

/// A call of a client, with an ID that the client chose. The action is beside the ID in one
/// object, so no action has a field named `id` or `call`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Call {
    pub id: u64,
    #[serde(flatten)]
    pub action: Action,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "call", rename_all = "snake_case")]
pub enum Action {
    /// The threads that the runner started, with their state.
    List,
    /// The updates of each thread from now on, after the updates that the runner keeps of it.
    Watch,
    /// A new thread in a folder that the person chose at this computer (THR-041). An empty
    /// `folder` is "No project": the runner makes a new empty folder for the thread.
    Start {
        program: Program,
        folder: PathBuf,
        #[serde(default)]
        mode: Mode,
        model: Option<String>,
        effort: Option<Effort>,
        /// A speed of the model other than the standard one: one of its `speeds` (TH117).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speed: Option<String>,
        prompt: String,
        /// A different copy of the program than the first on the `PATH` of the login: one of
        /// the copies the runner finds (TH97).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        program_path: Option<PathBuf>,
        /// The key of the draft whose files go with the first prompt (TH111).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        draft: Option<Uuid>,
        /// The files of the draft for the first prompt, in the order of the person.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        attachments: Vec<Uuid>,
        /// "New worktree": the runner makes a worktree of its own for the thread from the
        /// repository at `folder`, and starts the program there (TC7, TH116).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        worktree: bool,
        /// The branch that the worktree starts from; `HEAD` of the project when `None`.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        base: Option<String>,
    },
    /// A prompt to a thread: to its running turn, or as a new turn, with the files that the
    /// person attached for it (TH111).
    Send {
        thread: Uuid,
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        attachments: Vec<Uuid>,
        /// Codex: skills of the last `commands` answer of the thread (TH117).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        skills: Vec<crate::drive::ChosenSkill>,
    },
    /// The answer of the person to a request of the program (THR-030).
    Answer {
        thread: Uuid,
        request: Uuid,
        answer: Answer,
    },
    /// Stops the running turn; the program stays (THR-049).
    Interrupt { thread: Uuid },
    /// Ends the program of the thread and each process below it (THR-049).
    Stop { thread: Uuid },
    /// The person raised the soft limit (THR-114).
    RaiseLimit,
    /// What each program of this computer said of its account (THR-132). Only for a client of
    /// the same user: the name of an account is often an address of the person.
    Accounts,
    /// What the project files of a folder make the program do with no question, to show before
    /// a thread starts there (THR-046).
    Folder { folder: PathBuf },
    /// How the person lists each thread (TH46).
    Listings,
    /// A change of how the person lists a thread (TH46): only the person makes it.
    ChangeListing { thread: Uuid, change: ListingChange },
    /// The newest list of models that each program named (TH104).
    Models,
    /// The commands and the skills of a program (TH117): of a thread, or of a program in a
    /// folder before any thread runs there. Give `thread`, or `program` and `folder`.
    Commands {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thread: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        program: Option<Program>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        folder: Option<PathBuf>,
    },
    /// Codex compacts the context of a thread (TH117). Claude Code takes `/compact` as a prompt.
    Compact { thread: Uuid },
    /// The updates that the runner keeps of a thread with a number below `before`, at most
    /// `most` of them, the newest of them (TH106).
    Earlier {
        thread: Uuid,
        before: u64,
        most: usize,
    },
    /// The projects of this computer (TH107).
    Projects,
    /// A change of the projects (TH107): only the person makes it.
    ChangeProject { change: ProjectChange },
    /// The files and folders of the working folder of a thread that match `query`, at most
    /// `most`, which is at most 200 (TH108): the `@` list of the composer. Only the person asks it.
    Files {
        thread: Uuid,
        #[serde(default)]
        query: String,
        most: usize,
    },
    /// The folders in one folder of this computer, so that the person can choose a project by
    /// browsing (TH113): names only, never a file, a link or what a folder holds. Hidden folders
    /// only when `hidden`. Only the person asks it.
    Folders {
        folder: PathBuf,
        #[serde(default)]
        hidden: bool,
    },
    /// A prompt that waits for the running turn to end, then goes as the next turn (TH109).
    Queue { thread: Uuid, text: String },
    /// New words for a prompt that waits (TH109).
    Requeue {
        thread: Uuid,
        queued: Uuid,
        text: String,
    },
    /// Takes a prompt that waits out of the queue (TH109).
    Unqueue { thread: Uuid, queued: Uuid },
    /// Sends a prompt that waits now: into the running turn, or as the next turn (TH109).
    SendQueued { thread: Uuid, queued: Uuid },
    /// Changes the model, the effort or the mode of a thread (TH110): the row says it when the
    /// program has it.
    Set {
        thread: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        model: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        effort: Option<Effort>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mode: Option<Mode>,
        /// A speed of the model: one of its `speeds`, or `default` for the standard one (TH117).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        speed: Option<String>,
    },
    /// The first part of a file for a prompt, in base64 (TH111): the whole file has `size`
    /// bytes. Only the person attaches.
    Attach {
        thread: Uuid,
        name: String,
        mime: String,
        size: u64,
        bytes: String,
        /// `thread` is the key of a draft that the page made, not a thread: the files wait for
        /// the start that names it (TH111).
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        draft: bool,
    },
    /// The next part of a file that is not complete yet, in base64 (TH111).
    AttachMore {
        thread: Uuid,
        attachment: Uuid,
        bytes: String,
    },
    /// Drops a file that did not go with a prompt yet (TH111).
    Unattach { thread: Uuid, attachment: Uuid },
    /// An action on the worktree of one repository of a thread (TC7, TH116): the Git engine of
    /// the runner does it.
    Git {
        thread: Uuid,
        repository: String,
        #[serde(flatten)]
        action: crate::git::GitAction,
    },
    /// The version of the daemon of the runner (HMD-055). A runner from before this call closes
    /// the connection at it, which says that it is older.
    Version,
}

/// A frame of the runner to a client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "frame", rename_all = "snake_case")]
pub enum Reply {
    Done { id: u64, result: Outcome },
    Update(Box<Update>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    Ok,
    Started {
        thread: Uuid,
        /// The folder of the thread: for a start with no project, the new folder that the
        /// runner made for it (THR-041).
        #[serde(default)]
        folder: PathBuf,
    },
    Threads {
        threads: Vec<ThreadRow>,
        /// Words for the person while the holder is older than the daemon (TH98).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        holder: Option<String>,
    },
    Limits {
        limits: Limits,
    },
    Accounts {
        accounts: Vec<AccountRow>,
    },
    /// The notice of a folder, as lines for the person; empty when there is nothing to say.
    Folder {
        notice: Vec<String>,
    },
    /// The program of a stopped thread ended; `all_ended` is false when a process below it was
    /// still there or the walk could not run (THR-049).
    Stopped {
        all_ended: bool,
    },
    /// How the person lists each thread that has a listing (TH46).
    Listings {
        listings: Vec<Listing>,
    },
    /// The commands and the skills of a program (TH117). `pending` when the program did not
    /// name them yet: ask again later.
    Commands {
        #[serde(flatten)]
        lists: crate::drive::CommandLists,
        pending: bool,
    },
    Models {
        models: Vec<ModelRow>,
        /// Where the list of each program stands (TH117).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        programs: Vec<ProgramState>,
    },
    /// A file for a prompt took a part: it is complete when `received` is its size (TH111).
    Attached {
        id: Uuid,
        received: u64,
    },
    /// Oldest first; empty when the runner keeps nothing older.
    Earlier {
        updates: Vec<Update>,
    },
    Projects {
        projects: Vec<Project>,
    },
    /// The best match first (TH108); `more` when more paths matched than were asked.
    Files {
        paths: Vec<FoundPath>,
        #[serde(default)]
        more: bool,
    },
    /// The folders in `folder`, by name; `folder` is its real path. `more` when it held more
    /// than the answer gives (TH113).
    Folders {
        folder: PathBuf,
        folders: Vec<String>,
        #[serde(default)]
        more: bool,
    },
    /// The answer of the Git engine to [`Action::Git`].
    Git {
        #[serde(flatten)]
        result: crate::git::GitOutcome,
    },
    /// The version of the daemon of the runner, as its `--version` prints it after the name.
    Version {
        version: String,
    },
    Refused {
        why: Refusal,
    },
}

/// What a thread does now, by the order of trust of THR-085: the end of the process first, then
/// the stream of the program.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Starting,
    Working,
    /// A request waits for the person; only an answer, a stop or the end of the process clears
    /// it (THR-086, THR-087).
    NeedsYou,
    Idle,
    /// The program ended; `Stopped` tells why in [`ThreadRow::ended`].
    Stopped,
    /// No source can tell the state (THR-086).
    Unknown,
}

/// One thread of the runner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThreadRow {
    pub id: Uuid,
    pub program: Program,
    pub folder: PathBuf,
    pub mode: Mode,
    pub model: Option<String>,
    pub state: RunState,
    /// The session ID of Claude Code or the thread ID of Codex, when the program named it.
    pub session: Option<String>,
    pub title: String,
    pub started: chrono::DateTime<chrono::Utc>,
    /// Why the thread stopped, in words for the person.
    pub ended: Option<String>,
    /// The requests that wait for the person.
    pub open: Vec<Uuid>,
    /// The prompts that wait for the turn to end, oldest first (TH109). In the memory of the
    /// runner only: the record never holds them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub queued: Vec<Queued>,
    /// How the last turn ended, when one ended (TH109).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_turn: Option<LastTurn>,
    /// The effort of the model, when the person chose one (TH110).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<Effort>,
    /// The speed of the model, when the person chose one other than the standard (TH117).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<String>,
    /// Where the thread works, read at its start (TH112).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<Workspace>,
    /// The use of tokens that the program last said (TH112). It also comes live, as a
    /// [`DriveEvent::Usage`]; the row keeps it for a client that comes later.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

/// One update of one thread, in the order the runner made them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Update {
    pub thread: Uuid,
    pub seq: u64,
    #[serde(flatten)]
    pub change: Change,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum Change {
    /// Something the program said. A request of the program comes as [`Change::Asked`].
    Said { event: DriveEvent },
    /// The program asks the person; `request` is the ID of the runner for the answer.
    Asked { request: Uuid, detail: Request },
    /// A request is closed: answered, or gone with its process or turn.
    Closed { request: Uuid, why: String },
    /// A prompt the runner sent, with where it came from (THR-042).
    Prompt {
        text: String,
        source: String,
        /// How many images went with the prompt, and the names of the other files (TH111).
        #[serde(default, skip_serializing_if = "attach::is_zero")]
        images: u32,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        files: Vec<String>,
    },
    /// The state of a thread changed. The row is boxed: it is the largest part of a change.
    State {
        state: RunState,
        row: Box<ThreadRow>,
    },
}

#[cfg(test)]
mod call_tests;
