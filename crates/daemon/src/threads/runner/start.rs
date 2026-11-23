//! The start of a new thread (TH11, THR-041, THR-114): the checks of the runner, the count
//! against the soft limit, the start in the holder and its answer.

use std::path::{Path, PathBuf};

use agent_threads::control::{Refusal, RunState, ThreadRow, Workspace};
use agent_threads::drive::{Launch, Model, Program, SessionStart};
use uuid::Uuid;

use super::calls::codex_of;
use super::socket::Caller;
use super::thread::Thread;
use super::{Runner, Waits, login};

/// The shortest time between two asks of the login shell after a program was not found.
const ASK_AGAIN: std::time::Duration = std::time::Duration::from_secs(60);
use crate::threads::wire;

mod new_thread;
mod scratch;
pub(super) use crate::threads::notice::chosen_folder;
pub(super) use new_thread::NewThread;

/// The starts whose copy is being asked for its version (TH97).
pub(super) struct Asks {
    pub sender: tokio::sync::mpsc::Sender<(u64, super::silence::Answers)>,
    pub receiver: Option<tokio::sync::mpsc::Receiver<(u64, super::silence::Answers)>>,
    open: std::collections::BTreeMap<u64, Asking>,
    next: u64,
}

impl Default for Asks {
    fn default() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel(16);
        Self {
            sender,
            receiver: Some(receiver),
            open: std::collections::BTreeMap::new(),
            next: 0,
        }
    }
}

/// A start that waits for the answer of its copy.
struct Asking {
    new: NewThread,
    back: Caller,
    path: PathBuf,
    copies: Vec<PathBuf>,
}

impl Runner {
    /// Checks a new thread, counts it against the soft limit (THR-114) and asks the holder to
    /// start its program; the answer goes to the client when the holder answers.
    pub(super) async fn start(&mut self, new: NewThread, back: Caller) -> Result<(), Refusal> {
        // A start whose copy is being asked counts against the soft limit too.
        self.may_start(u32::try_from(self.asks.open.len()).unwrap_or(u32::MAX))?;
        let no_project = new.folder.as_os_str().is_empty();
        if !no_project {
            chosen_folder(&new.folder, self.dir.parent().unwrap_or(&self.dir))?;
        }
        self.draft_ready(&new)?;
        if !self
            .said
            .models
            .offers(new.program, new.model.as_deref(), new.effort)
        {
            return Err(Refusal::BadValue);
        }
        if login::find(new.program, &self.env, login::executable, login::trusted).is_none() {
            self.ask_login_again(new.program).await;
        }
        let copies = login::copies(new.program, &self.env, login::executable, login::trusted);
        let path = login::chosen_copy(new.program_path.as_deref(), &copies)?;
        match self.silence.kept(&path) {
            Some(true) => {
                let asking = Asking {
                    new,
                    back,
                    path,
                    copies,
                };
                self.start_checked(asking).await
            }
            Some(false) => Err(self.silence.refusal(new.program, &path, &copies)),
            None => {
                // The ask runs away from the loop; the start goes on when it answers (TH97).
                // One ask for each copy at a time: a later start waits for the same answer.
                let asked = self.asks.open.values().any(|open| open.path == path);
                let env = login::with_folder_of(self.env.clone(), &path);
                let ask = super::silence::ask(path.clone(), copies.clone(), env);
                let sender = self.asks.sender.clone();
                let id = self.asks.next;
                self.asks.next += 1;
                self.asks.open.insert(
                    id,
                    Asking {
                        new,
                        back,
                        path,
                        copies,
                    },
                );
                if !asked {
                    tokio::spawn(async move {
                        // A runner that ended does not need the answer.
                        let _ = sender.send((id, ask.await)).await;
                    });
                }
                Ok(())
            }
        }
    }

    /// The answers of the copies for the start `id`: it goes on, or it is refused with words.
    /// Each start that waited for that copy goes on in its order, or is refused with words.
    pub(super) async fn asked(&mut self, id: u64, answers: super::silence::Answers) {
        self.silence.keep(&answers);
        let Some(path) = self.asks.open.get(&id).map(|asking| asking.path.clone()) else {
            return;
        };
        // The answer itself decides: a copy replaced while it was asked keeps this answer.
        let answered = answers
            .iter()
            .any(|(copy, version)| *copy == path && version.is_some());
        let waiting: Vec<u64> = self
            .asks
            .open
            .iter()
            .filter(|(_, asking)| asking.path == path)
            .map(|(id, _)| *id)
            .collect();
        for id in waiting {
            let Some(asking) = self.asks.open.remove(&id) else {
                continue;
            };
            let back = asking.back.clone();
            let refusal = if answered {
                self.start_checked(asking).await.err()
            } else {
                Some(
                    self.silence
                        .refusal(asking.new.program, &asking.path, &asking.copies),
                )
            };
            if let Some(why) = refusal {
                self.note(
                    super::log::Kind::Refusal,
                    None,
                    super::calls::refusal_word(&why),
                );
                back.answer(super::holder_events::refused(why));
            }
        }
    }

    /// The threads whose program has not stopped, which count against the soft limit.
    pub(super) fn running(&self) -> u32 {
        let count = self
            .threads
            .values()
            .filter(|thread| thread.row.state != RunState::Stopped)
            .count();
        u32::try_from(count).unwrap_or(u32::MAX)
    }

    /// Whether a new thread may start now: a holder that fits (THR-113), and fewer than the soft
    /// limit of running threads with the `asking` starts that wait for an answer (THR-114).
    fn may_start(&self, asking: u32) -> Result<(), Refusal> {
        if !self.holder.fits || self.holder.quitting {
            return match self.holder_words() {
                Some(said) => Err(Refusal::HolderOlder { said }),
                None => Err(Refusal::Holder),
            };
        }
        if self.running().saturating_add(asking) >= self.limits.running {
            return Err(Refusal::OverLimit {
                limits: self.limits,
            });
        }
        Ok(())
    }

    /// The start of a thread whose copy answered its version.
    /// A start with no project gets its new folder here, after each check that needs no folder,
    /// and loses it again when a later check refuses the start (THR-041).
    async fn start_checked(&mut self, mut asking: Asking) -> Result<(), Refusal> {
        let no_project = asking.new.folder.as_os_str().is_empty();
        if no_project {
            asking.new.folder = scratch::made(&self.env, chrono::Local::now().date_naive())?;
        }
        let folder = asking.new.folder.clone();
        let started = self.start_in(asking).await;
        if no_project && started.is_err() {
            scratch::unmade(&folder);
        }
        started
    }

    async fn start_in(&mut self, asking: Asking) -> Result<(), Refusal> {
        let Asking {
            new,
            back,
            path,
            copies,
        } = asking;
        // The holder or the count can change while the copy is asked.
        self.may_start(0)?;
        let id = Uuid::now_v7();
        let (row, action) = self.prepared(&new, id, &path)?;
        let daemon = self.dir.parent().unwrap_or(&self.dir);
        let mut notice = crate::threads::notice::before_start(&row.folder, daemon);
        let named = new.program_path.is_some();
        notice.extend(super::silence::chosen(new.program, &path, named, &copies));
        self.silence.started(id, new.program, path);
        let codex = codex_of(&row, new.effort, daemon);
        self.threads.insert(id, Thread::new(row, new.effort, codex));
        if !notice.is_empty() {
            // The thread keeps the notice as its first line (THR-046).
            let text = notice.join("\n");
            let event = agent_threads::drive::DriveEvent::Notice { text };
            self.publish(id, agent_threads::control::Change::Said { event });
        }
        let Some(command) = self.command(action).await else {
            self.threads.remove(&id);
            return Err(Refusal::Holder);
        };
        let files = self.adopt_draft(&new, id);
        let prompt = new.prompt;
        self.waits.insert(
            command,
            Waits::Start {
                thread: id,
                back,
                prompt,
                files,
            },
        );
        Ok(())
    }

    /// Ends each thread that is still starting after [`super::silence::FIRST_ANSWER`]; its end
    /// says which program at which path said nothing (TH97).
    pub(super) async fn end_the_silent(&mut self) {
        let threads: Vec<_> = self
            .threads
            .values()
            .map(|thread| (thread.row.id, thread.row.state, thread.row.started))
            .collect();
        for id in self.silence.overdue(&threads, chrono::Utc::now()) {
            let stop = wire::Action::Stop {
                thread: id.to_string(),
            };
            if self.command(stop).await.is_some() {
                self.note(super::log::Kind::Stop, Some(id), "no first answer");
            }
        }
    }

    /// Asks the login shell again for its environment, at most once in [`ASK_AGAIN`]: it may not
    /// have answered when the daemon started on a busy computer. The answer is kept only when it
    /// finds the program, so that a shell that does not answer does not take a good environment.
    async fn ask_login_again(&mut self, program: Program) {
        if self.asked_again.is_some_and(|at| at.elapsed() < ASK_AGAIN) {
            return;
        }
        self.asked_again = Some(std::time::Instant::now());
        let env = login::environment(&self.daemon_env).await;
        if login::find(program, &env, login::executable, login::trusted).is_some() {
            self.env = env;
        }
    }

    /// The row of a new thread and the start for the holder: the folder that the person chose
    /// (THR-041), the program on the `PATH` of the login, the arguments of the closed list
    /// (THR-040) and the environment of the login (R-308).
    fn prepared(
        &self,
        new: &NewThread,
        id: Uuid,
        path: &Path,
    ) -> Result<(ThreadRow, wire::Action), Refusal> {
        let folder = chosen_folder(&new.folder, self.dir.parent().unwrap_or(&self.dir))?;
        let model = new
            .model
            .as_deref()
            .map(Model::new)
            .transpose()
            .map_err(|_| Refusal::BadValue)?;
        let launch = Launch {
            program: new.program,
            path: path.to_owned(),
            mode: new.mode,
            model: model.clone(),
            effort: new.effort,
            session: SessionStart::New(id),
        };
        let invocation = launch.invocation().map_err(|_| Refusal::BadValue)?;
        let mut env = login::with_folder_of(self.env.clone(), path);
        if new.program == Program::ClaudeCode {
            env.extend(
                login::CLAUDE_VARIABLES.map(|(name, value)| (name.to_owned(), value.to_owned())),
            );
        }
        let row = ThreadRow {
            id,
            program: new.program,
            folder: folder.clone(),
            mode: new.mode,
            model: model.as_ref().map(|model| model.as_str().to_owned()),
            state: RunState::Starting,
            session: None,
            title: String::new(),
            started: chrono::Utc::now(),
            ended: None,
            open: Vec::new(),
            queued: Vec::new(),
            last_turn: None,
            effort: new.effort,
            workspace: Some(Workspace::of(&folder)),
            usage: None,
        };
        let action = wire::Action::Start {
            thread: id.to_string(),
            program: invocation.program.to_string_lossy().into_owned(),
            args: invocation.args,
            env,
            folder: folder.to_string_lossy().into_owned(),
        };
        Ok((row, action))
    }
}
