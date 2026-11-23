//! The thread runner (TH11, THR-020 to THR-051): it starts Claude Code and Codex in the holder,
//! drives them over their pipes, keeps the requests that wait for the person, and serves the
//! clients of this computer on its local socket. Nothing of a thread leaves the computer in
//! stage 1b.
//!
//! Pure: [`thread`] (one thread) and [`codex`] (the calls to a Codex server). I/O: this file
//! (the loop and the holder), [`holder_events`], [`calls`] (the calls of the clients),
//! [`socket`], [`login`], [`record`] and [`log`].

mod accounts;
mod attachments;
mod calls;
pub mod codex;
mod git;
mod grok;
mod holder_events;
pub mod launcher;
mod listings;
pub mod log;
pub mod login;
mod models;
pub mod peer;
pub mod record;
mod silence;
pub mod socket;
mod start;
#[cfg(test)]
mod tests;
pub mod thread;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_threads::control::{Change, Limits, Reply};
use tokio::sync::mpsc;
use uuid::Uuid;

use super::link::{self, HolderLink, LinkError};
use super::wire::{Action, Command};
use log::Kind;
use socket::{Caller, Client};
use thread::{Step, Thread};

/// The most stopped threads that the runner keeps; the oldest go first.
const STOPPED_KEPT: usize = 50;

#[derive(Debug, thiserror::Error)]
pub enum RunnerError {
    #[error("the folder of the threads: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Holder(#[from] LinkError),
    #[error("a different runner holds the threads of this daemon")]
    Busy,
}

/// What a command to the holder was for, so that its answer goes to the right place.
#[derive(Debug)]
enum Waits {
    Start {
        thread: Uuid,
        back: Caller,
        prompt: String,
        /// The files of the draft, now of the thread, for the first prompt (TH111).
        files: Vec<Uuid>,
    },
    Stop {
        back: Caller,
    },
    /// The last line of a prompt; the prompt is shown as sent when the holder takes it.
    Send(calls::SentPrompt),
    /// The line of an answer; the request closes when the holder takes it (THR-030).
    Answer(Pending),
}

/// An answer that waits for the holder to take its line.
#[derive(Debug)]
struct Pending {
    thread: Uuid,
    request: Uuid,
    word: &'static str,
    back: Caller,
}

/// Why steps of a thread did not all go.
#[derive(Debug)]
enum NotSent {
    /// The link to the holder was lost while a command was sent.
    LinkDown,
    /// The record was not written, so a turn that leaves the profile of the folder of Anima did
    /// not go (THR-041); the other steps went.
    Held,
}

/// The runner: its threads, its link to the holder and its clients.
pub struct Runner {
    dir: PathBuf,
    program: PathBuf,
    link: HolderLink,
    /// What the runner knows of the holder (THR-113, TH98).
    holder: holder_events::Standing,
    env: Vec<(String, String)>,
    /// The environment of the daemon, to ask the login shell again when a program is not found.
    daemon_env: Vec<(String, String)>,
    /// When the login shell was last asked again after a program was not found.
    asked_again: Option<std::time::Instant>,
    threads: BTreeMap<Uuid, Thread>,
    limits: Limits,
    /// How the person lists each thread (TH46).
    listings: BTreeMap<Uuid, agent_threads::control::Listing>,
    /// The projects of this computer, in the order the person added them (TH107).
    projects: Vec<agent_threads::control::Project>,
    watchers: Vec<Client>,
    waits: BTreeMap<u64, Waits>,
    /// The commands that change a program and that the holder did not answer yet: a new link
    /// sends them again with their own IDs, and the holder does each one time (H10).
    unanswered: BTreeMap<u64, Command>,
    /// The last line of each thread that the runner read, so that a line the holder gives
    /// again is not read twice.
    read_up_to: BTreeMap<Uuid, u64>,
    log: log::RunnerLog,
    /// How many refusals the daemon's log took this minute (MET-019).
    refusal_lines: calls::RefusalLines,
    /// The copies that were asked for their version and the threads that wait for their first
    /// line (TH97).
    silence: silence::Silence,
    /// The starts whose copy is being asked for its version.
    asks: start::Asks,
    /// When each thread last got the words of a lost sign-in (TH99).
    sign_in_told: BTreeMap<Uuid, std::time::Instant>,
    /// What the programs said of their accounts (THR-051) and their models (TH104): in memory
    /// only.
    said: accounts::Said,
    model_asks: models::Asks,
    /// The skills of Codex in the folders of new threads (TH117): in memory only.
    folder_skills: calls::FolderSkills,
    /// The last walk of the folder of each thread, for the `@` list (TH108): in memory only.
    walked: calls::Walked,
    /// The files that the person attached for a prompt (TH111).
    attached: attachments::Attachments,
    /// What the launcher of the Git engine knows of the threads that run.
    rights: launcher::SharedRights,
    /// The Git engine of the threads (TH116).
    git: git::Gits,
}

/// Whether the daemon in `daemon_dir` serves a person, so that it may run agents in the login
/// of that person: never the system unit, which runs as an account of its own (THR-128).
#[must_use]
pub fn serves_a_person(daemon_dir: &Path) -> bool {
    daemon_dir != Path::new(crate::units::plan::SYSTEM_HOME) && !rustix::process::getuid().is_root()
}

/// Runs the runner of the daemon in `daemon_dir` until `stop` completes. `program` is the path
/// of `anima-daemon`, which the runner starts as the holder when none answers; `daemon_env` is
/// the environment of the daemon, from which the runner asks the login shell for the
/// environment of the agents. The programs of the threads stay in the holder when the runner
/// ends (THR-113).
///
/// # Errors
/// [`RunnerError`] when the folder, its lock, the record or the socket cannot be set up.
pub async fn run(
    daemon_dir: &Path,
    program: &Path,
    daemon_env: Vec<(String, String)>,
    stop: impl Future<Output = ()>,
) -> Result<(), RunnerError> {
    let dir = link::folder(daemon_dir);
    // A socket path has a limit of about 100 bytes (104 on a Mac, 108 on Linux).
    if dir.join(agent_threads::control::SOCKET).as_os_str().len() > 100 {
        return Err(RunnerError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the folder of the daemon is too long for a socket path (more than 100 bytes)",
        )));
    }
    record::private_folder(&dir)?;
    let _lock = record::lock(&dir)?;
    let saved = record::read(&dir)?;
    let env = login::environment(&daemon_env).await;
    let (rights, gits) = git::Gits::make(&env, &daemon_env, &dir);
    tokio::pin!(stop);
    let link = tokio::select! {
        link = link::reach_patiently(program, &dir) => link,
        () = &mut stop => return Ok(()),
    };
    let mut runner = Runner {
        dir: dir.clone(),
        program: program.to_owned(),
        link,
        holder: holder_events::Standing::default(),
        env,
        daemon_env,
        asked_again: None,
        threads: BTreeMap::new(),
        limits: saved.limits,
        listings: saved
            .listings
            .into_iter()
            .map(|listing| (listing.thread, listing))
            .collect(),
        projects: saved.projects,
        watchers: Vec::new(),
        waits: BTreeMap::new(),
        unanswered: BTreeMap::new(),
        read_up_to: BTreeMap::new(),
        log: log::RunnerLog::at(&dir),
        refusal_lines: calls::RefusalLines::default(),
        silence: silence::Silence::default(),
        asks: start::Asks::default(),
        sign_in_told: BTreeMap::new(),
        walked: calls::Walked::default(),
        said: accounts::Said::default(),
        model_asks: models::Asks::default(),
        folder_skills: calls::FolderSkills::default(),
        attached: attachments::Attachments::at(&dir),
        rights,
        git: gits,
    };
    runner.restore(saved.threads);
    let listener = socket::bind(&dir)?;
    runner.serve(listener, &mut stop).await;
    if let Err(error) = std::fs::remove_file(dir.join(agent_threads::control::SOCKET)) {
        tracing::warn!(%error, "the socket of the runner was not removed");
    }
    runner.save();
    Ok(())
}

impl Runner {
    /// The loop of the runner: the holder, the clients, and the stop.
    async fn serve(&mut self, listener: tokio::net::UnixListener, stop: impl Future<Output = ()>) {
        let (sender, mut incoming) = mpsc::channel(256);
        let accepting = tokio::spawn(socket::accept(listener, sender, self.dir.clone()));
        tokio::pin!(stop);
        let mut look = tokio::time::interval(Duration::from_secs(10));
        let (Some(mut asked), Some(mut listed), Some(mut made)) = (
            self.asks.receiver.take(),
            self.model_asks.receiver.take(),
            self.asks.made.take(),
        ) else {
            return;
        };
        loop {
            tokio::select! {
                event = self.link.events.recv() => self.heard_holder(event).await,
                Some(call) = incoming.recv() => self.call(call).await,
                Some((id, answers)) = asked.recv() => self.asked(id, answers).await,
                Some((program, models)) = listed.recv() => self.models_asked(program, models),
                Some(worktree) = made.recv() => self.worktree_made(worktree).await,
                _ = look.tick() => self.look_around().await,
                () = &mut stop => break,
            }
        }
        accepting.abort();
    }

    /// An event of the holder, or a new link when the old one closed.
    async fn heard_holder(&mut self, event: Option<crate::threads::wire::Event>) {
        match event {
            Some(event) => self.holder_event(event).await,
            None => self.reconnect().await,
        }
    }

    /// What the runner looks at every ten seconds: the silent starts, the old files of prompts,
    /// an older holder, and the records of the Git engine.
    async fn look_around(&mut self) {
        self.end_the_silent().await;
        self.attached.expire(std::time::Instant::now());
        self.replace_older_holder().await;
        self.git_touch();
    }

    /// Does a step of a thread that is not a command to the holder.
    fn own_step(&mut self, id: Uuid, step: Step, last_write: Option<u64>) {
        let program = self.threads.get(&id).map(|thread| thread.row.program);
        match (step, program) {
            (Step::Log(what), _) => self.note(Kind::Stop, Some(id), what),
            (Step::Change(change), _) => self.publish(id, change),
            (Step::Queued(queued), _) => self.queued_went(id, queued, last_write),
            (Step::Account(facts), Some(program)) => {
                self.said.accounts.note(program, *facts, chrono::Utc::now());
            }
            (Step::Models(models), Some(program)) => {
                self.said.models.note(program, models, chrono::Utc::now());
            }
            // A fact of a thread that is gone has no program to keep it for; a command is never
            // here, as `apply` sends it to the holder.
            (Step::Account(_) | Step::Models(_), None)
            | (
                Step::Write(_)
                | Step::Mark
                | Step::EndChildren
                | Step::EndProgram
                | Step::Workspace,
                _,
            ) => {}
        }
    }

    /// Does the steps of a thread, in order. Gives the ID of the command of the last line
    /// written, [`NotSent::LinkDown`] when a command did not reach the holder, or
    /// [`NotSent::Held`] when a line was held back.
    async fn apply(&mut self, id: Uuid, steps: Vec<Step>) -> Result<Option<u64>, NotSent> {
        let key = id.to_string();
        let mut last_write = None;
        let (mut held, mut put_back) = (false, false);
        for step in steps {
            if let Step::Write(line) = &step
                && self.held_back(id, line)
            {
                held = true;
                continue;
            }
            if let (true, Step::Queued(queued)) = (held, &step) {
                put_back |= self.waits_first(id, queued);
                continue;
            }
            // The row of the queue after the prompt left it is not true now: the row of the
            // put back is.
            if put_back && matches!(step, Step::Change(Change::State { .. })) {
                continue;
            }
            let Some(action) = step.command(&key) else {
                if step == Step::Workspace {
                    self.look_at_workspace(id).await;
                    self.git_turn_ended(id);
                }
                self.own_step(id, step, last_write);
                continue;
            };
            let write = matches!(action, Action::Write { .. });
            let command = self.command(action).await.ok_or(NotSent::LinkDown)?;
            if write {
                last_write = Some(command);
            }
        }
        if held {
            return Err(NotSent::Held);
        }
        Ok(last_write)
    }

    /// Gives a change of a thread its number and sends it to each watcher; a watcher that does
    /// not read is let go (THR-045).
    fn publish(&mut self, id: Uuid, change: agent_threads::control::Change) {
        let Some(thread) = self.threads.get_mut(&id) else {
            return;
        };
        // A row that changed can change the rights of the launcher: a mode, a stop.
        let row = matches!(change, agent_threads::control::Change::State { .. });
        let reply = Reply::Update(Box::new(thread.note(change)));
        self.rights_if(row);
        self.watchers
            .retain(|watcher| socket::tell(watcher, &reply));
    }

    /// Sends a command to the holder; a closed link is found by the loop and connected again.
    /// A command that changes a program is kept until the holder answers it, and goes again on
    /// the new link; any other is lost with the link (`None`).
    async fn command(&mut self, action: Action) -> Option<u64> {
        let kept = matches!(
            action,
            Action::Write { .. }
                | Action::Stop { .. }
                | Action::EndChildren { .. }
                | Action::Mark { .. }
        );
        let command = self.link.prepare(action);
        let id = command.id;
        let sent = self.link.resend(&command).await;
        if kept {
            self.unanswered.insert(id, command);
        }
        match sent {
            Ok(()) => Some(id),
            Err(error) => {
                tracing::warn!(%error, "the holder did not take a command");
                kept.then_some(id)
            }
        }
    }
}
