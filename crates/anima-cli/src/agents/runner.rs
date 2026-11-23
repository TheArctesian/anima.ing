//! The local socket of this computer's thread runner (TH12, THR-128); nothing goes to the network.

use std::os::unix::fs::{FileTypeExt as _, MetadataExt as _, PermissionsExt as _};
use std::path::PathBuf;

use agent_threads::control::{self, Action, Call, Outcome, Refusal, Reply};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader, Lines};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};

use crate::failure::{Failure, Hint};

pub struct RunnerLink {
    lines: Lines<BufReader<OwnedReadHalf>>,
    write: OwnedWriteHalf,
    next: u64,
}

fn not_running() -> Failure {
    Failure::NotFound {
        message: "The thread runner of this computer is not running.".to_owned(),
        hint: Some(Hint::run("anima-daemon run")),
    }
}

fn socket() -> Option<PathBuf> {
    let folder = control::folder(|name| std::env::var(name).ok(), cfg!(target_os = "macos"))?;
    Some(folder.join(control::SOCKET))
}

impl RunnerLink {
    /// Connects only to a private socket of this user (THR-128).
    ///
    /// # Errors
    /// [`Failure::NotFound`] when no runner listens; [`Failure::Forbidden`] for a socket that is not private.
    pub async fn connect() -> Result<Self, Failure> {
        let socket = socket().ok_or_else(not_running)?;
        let meta = std::fs::symlink_metadata(&socket).map_err(|_| not_running())?;
        let ours = meta.uid() == rustix::process::getuid().as_raw()
            && meta.permissions().mode().trailing_zeros() >= 6
            && meta.file_type().is_socket();
        if !ours {
            return Err(Failure::Forbidden {
                message: format!("{} is not a private socket of this user.", socket.display()),
                hint: None,
            });
        }
        let stream = UnixStream::connect(&socket)
            .await
            .map_err(|_| not_running())?;
        let (read, write) = stream.into_split();
        Ok(Self {
            lines: BufReader::new(read).lines(),
            write,
            next: 0,
        })
    }

    /// # Errors
    /// [`Failure::NotFound`] when the runner is gone.
    pub async fn send(&mut self, action: Action) -> Result<u64, Failure> {
        self.next += 1;
        let call = Call {
            id: self.next,
            action,
        };
        let mut text = serde_json::to_string(&call).map_err(|error| Failure::Write {
            reason: error.to_string(),
        })?;
        text.push('\n');
        self.write
            .write_all(text.as_bytes())
            .await
            .map_err(|_| not_running())?;
        Ok(self.next)
    }

    /// # Errors
    /// [`Failure::Input`] for a frame that does not read.
    pub async fn next(&mut self) -> Result<Option<Reply>, Failure> {
        let Some(line) = self.lines.next_line().await.map_err(|_| not_running())? else {
            return Ok(None);
        };
        serde_json::from_str(&line)
            .map(Some)
            .map_err(|error| Failure::Input {
                what: "a frame of the thread runner".to_owned(),
                reason: error.to_string(),
            })
    }

    /// # Errors
    /// As [`Self::send`] and [`Self::next`].
    /// [`Failure::Conflict`] when the runner closed the connection at the call because its daemon
    /// is older than this program and does not know the call.
    pub async fn call(&mut self, action: Action) -> Result<Outcome, Failure> {
        let id = self.send(action).await?;
        loop {
            match self.next().await? {
                Some(Reply::Done { id: done, result }) if done == id => return Ok(result),
                Some(_) => {}
                None => return Err(closed().await),
            }
        }
    }

    /// The version of the daemon of the runner; `None` when the runner closed the connection at
    /// the call, as a runner from before it does.
    async fn version(&mut self) -> Result<Option<String>, Failure> {
        let id = self.send(Action::Version).await?;
        while let Some(reply) = self.next().await? {
            if let Reply::Done { id: done, result } = reply
                && done == id
            {
                return Ok(match result {
                    Outcome::Version { version } => Some(version),
                    _ => None,
                });
            }
        }
        Ok(None)
    }
}

/// Why a runner closed the connection with no answer: it asks a new connection for the version
/// of the daemon. A runner that closes that one too, or names an older version, is older than
/// this program and did not know the call (HMD-055).
async fn closed() -> Failure {
    let Ok(mut link) = RunnerLink::connect().await else {
        return not_running();
    };
    let this = env!("CARGO_PKG_VERSION");
    match link.version().await {
        Ok(None) => older(None, this),
        Ok(Some(version)) if numbers(&version) < numbers(this) => older(Some(&version), this),
        _ => Failure::NotFound {
            message: "The thread runner of this computer ended the call with no answer.".to_owned(),
            hint: Some(Hint::run("anima agents running")),
        },
    }
}

/// The failure of a call that the daemon of this computer does not know, in words for the person.
#[must_use]
pub fn older(daemon: Option<&str>, this: &str) -> Failure {
    let versions = daemon.map_or_else(
        || format!("this app and command are {this}"),
        |daemon| format!("the daemon is {daemon}, this app and command are {this}"),
    );
    Failure::Conflict {
        message: format!(
            "The daemon on this computer is older than this app ({versions}): it does not know this call. Update it: quit Anima and open it again, which updates the daemon at its start; or install it again in the settings of this computer, at Metrics source: Daemon."
        ),
    }
}

/// The three numbers of a version, for an order; a version that does not read is the lowest.
fn numbers(version: &str) -> [u64; 3] {
    let mut parts = version.split('.').map(|part| part.parse().unwrap_or(0));
    [
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    ]
}

#[must_use]
pub fn refusal(why: &Refusal) -> Failure {
    let (message, hint) = match why {
        Refusal::UnknownThread => (
            "No thread of this computer has that ID.",
            Some(Hint::run("anima agents running")),
        ),
        Refusal::FolderNotPermitted => (
            "That folder is not an absolute folder of this computer.",
            None,
        ),
        Refusal::ProgramNotFound => (
            "That program is not on the PATH of your login, or that path is not a copy the runner finds.",
            None,
        ),
        Refusal::ProgramDoesNotAnswer { said, .. }
        | Refusal::HolderOlder { said }
        | Refusal::CannotChange { said }
        | Refusal::TooLarge { said }
        | Refusal::CannotAttach { said } => {
            return Failure::Forbidden {
                message: said.clone(),
                hint: None,
            };
        }
        Refusal::BadValue => (
            "A value is not permitted (a model name takes letters, digits, '.', '_', '-', '[' and ']').",
            None,
        ),
        Refusal::RequestGone => (
            "That request or queued prompt is gone: it was answered, closed or sent, or its program ended.",
            None,
        ),
        Refusal::AnswerDoesNotFit => ("That answer does not fit the request.", None),
        Refusal::NotRunning => ("The program of that thread is not running.", None),
        Refusal::Holder => ("The holder of the programs did not take the command.", None),
        Refusal::FromAnAgent => (
            "Only the person can answer a request, raise the limit, start a thread, change the list of threads, list the files of a thread, change the queue of a thread or change its model, effort or mode: this call came from a process of an agent.",
            None,
        ),
        Refusal::Running => (
            "The program of that thread still runs: stop it first, then remove it from the list.",
            Some(Hint::run("anima agents stop <thread>")),
        ),
        Refusal::QueueFull => (
            "That thread has 20 queued prompts: send or drop one first.",
            None,
        ),
        Refusal::OverLimit { .. } => (
            control::OVER_LIMIT,
            Some(Hint::run("anima agents raise-limit")),
        ),
        Refusal::Worktree { why } => return worktree_refused(why),
    };
    Failure::Forbidden {
        message: message.to_owned(),
        hint,
    }
}

/// Why the worktree of a new thread was not made, in words for the person (TH116).
fn worktree_refused(why: &agent_threads::git::GitRefusal) -> Failure {
    Failure::Forbidden {
        message: worktree_words(why),
        hint: None,
    }
}

fn worktree_words(why: &agent_threads::git::GitRefusal) -> String {
    use agent_threads::git::GitRefusal;
    let words = match why {
        GitRefusal::Failed { said } => return format!("The worktree was not made: {said}"),
        GitRefusal::NotARepository => "That folder is not the top folder of a Git repository.",
        GitRefusal::UnknownBase => "That branch or commit is not in the repository.",
        GitRefusal::GitNotFound => "git is not on the PATH of your login.",
        GitRefusal::NotOnWindows => "Worktrees are not available on Windows yet.",
        GitRefusal::RecordsHeld => {
            "A thread runs in a folder that holds the records of Anima's Git: stop that thread first."
        }
        _ => "The worktree of the thread was not made.",
    };
    words.to_owned()
}
