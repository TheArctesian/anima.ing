//! The start of a new thread (TH11, THR-041, THR-114): the checks of the runner, the count
//! against the soft limit, the start in the holder and its answer.

use std::path::PathBuf;

use agent_threads::control::{Refusal, RunState};
use agent_threads::drive::Program;
use uuid::Uuid;

use super::calls::codex_of;
use super::socket::Caller;
use super::thread::Thread;
use super::{Runner, Waits, login};

/// The shortest time between two asks of the login shell after a program was not found.
const ASK_AGAIN: std::time::Duration = std::time::Duration::from_secs(60);
use crate::threads::wire;

mod new_thread;
pub(super) mod private_tmp;
mod scratch;
mod worktree;
pub(super) use crate::threads::notice::chosen_folder;
pub(super) use new_thread::NewThread;
pub(super) use worktree::WorktreeMade;

/// The starts whose copy is being asked for its version (TH97).
pub(super) struct Asks {
    pub sender: tokio::sync::mpsc::Sender<(u64, super::silence::Answers)>,
    pub receiver: Option<tokio::sync::mpsc::Receiver<(u64, super::silence::Answers)>>,
    open: std::collections::BTreeMap<u64, Asking>,
    next: u64,
    /// The starts whose worktree is being made, by the ID of their thread (TH116).
    making: std::collections::BTreeMap<Uuid, Asking>,
    worktrees: tokio::sync::mpsc::Sender<WorktreeMade>,
    pub made: Option<tokio::sync::mpsc::Receiver<WorktreeMade>>,
}

impl Asks {
    /// Whether the worktree of the start of `thread` is being made.
    pub(super) fn making(&self, thread: Uuid) -> bool {
        self.making.contains_key(&thread)
    }
}

impl Default for Asks {
    fn default() -> Self {
        let (sender, receiver) = tokio::sync::mpsc::channel(16);
        let (worktrees, made) = tokio::sync::mpsc::channel(16);
        Self {
            sender,
            receiver: Some(receiver),
            open: std::collections::BTreeMap::new(),
            next: 0,
            making: std::collections::BTreeMap::new(),
            worktrees,
            made: Some(made),
        }
    }
}

/// A start that waits for the answer of its copy.
struct Asking {
    new: NewThread,
    back: Caller,
    path: PathBuf,
    copies: Vec<PathBuf>,
    /// The thread, when it was decided before the start: its worktree is made first.
    id: Option<Uuid>,
}

impl Runner {
    /// Checks a new thread, counts it against the soft limit (THR-114) and asks the holder to
    /// start its program; the answer goes to the client when the holder answers.
    pub(super) async fn start(&mut self, new: NewThread, back: Caller) -> Result<(), Refusal> {
        self.validate_start(&new)?;
        if login::find(new.program, &self.env, login::executable, login::trusted).is_none() {
            self.ask_login_again(new.program).await;
        }
        let copies = login::copies(new.program, &self.env, login::executable, login::trusted);
        let path = login::chosen_copy(new.program_path.as_deref(), &copies)?;
        let asking = Asking {
            new,
            back,
            path,
            copies,
            id: None,
        };
        match self.silence.kept(&asking.path) {
            Some(true) => self.start_checked(asking).await,
            Some(false) => {
                Err(self
                    .silence
                    .refusal(asking.new.program, &asking.path, &asking.copies))
            }
            None => {
                // The ask runs away from the loop; the start goes on when it answers (TH97).
                // One ask for each copy at a time: a later start waits for the same answer.
                let asked = self.asks.open.values().any(|open| open.path == asking.path);
                let env = login::with_folder_of(self.env.clone(), &asking.path);
                let (path, copies) = (asking.path.clone(), asking.copies.clone());
                let ask = async move {
                    let load = super::silence::load().await;
                    super::silence::ask(path, copies, env, load).await
                };
                let sender = self.asks.sender.clone();
                let id = self.asks.next;
                self.asks.next += 1;
                self.asks.open.insert(id, asking);
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

    fn validate_start(&self, new: &NewThread) -> Result<(), Refusal> {
        // A start whose copy is being asked, or whose worktree is being made, counts against the
        // soft limit too (THR-114).
        let asking = self.asks.open.len() + self.asks.making.len();
        self.may_start(u32::try_from(asking).unwrap_or(u32::MAX))?;
        let no_project = new.folder.as_os_str().is_empty();
        if !no_project {
            chosen_folder(&new.folder, self.dir.parent().unwrap_or(&self.dir))?;
        }
        self.draft_ready(new)?;
        if !self
            .said
            .models
            .offers(new.program, new.model.as_deref(), new.effort)
        {
            return Err(Refusal::BadValue);
        }
        Ok(())
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
            .copies
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
        if asking.new.worktree.is_some() && asking.id.is_none() {
            return self.make_worktree(asking);
        }
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
            id,
        } = asking;
        // The holder or the count can change while the copy is asked.
        self.may_start(0)?;
        let id = id.unwrap_or_else(Uuid::now_v7);
        let (row, action) = self.prepared(&new, id, &path)?;
        let daemon = self.dir.parent().unwrap_or(&self.dir);
        let mut notice = crate::threads::notice::before_start(&row.folder, daemon);
        let named = new.program_path.is_some();
        notice.extend(super::silence::chosen(new.program, &path, named, &copies));
        self.rights_copy(id, path.clone());
        self.silence.started(id, new.program, path);
        let codex = codex_of(&row, new.effort, daemon);
        self.threads.insert(id, Thread::new(row, new.effort, codex));
        // The hold of the Git engine counts the roots of this thread before its agent starts,
        // and no root of its sandbox is shared with a different running thread (THR-050).
        self.rights_now();
        if let Some(said) = self.shared_root(id) {
            self.threads.remove(&id);
            self.rights_now();
            return Err(Refusal::FolderShared { said });
        }
        self.say_priority();
        if !notice.is_empty() {
            // The thread keeps the notice as its first line (THR-046).
            let text = notice.join("\n");
            let event = agent_threads::drive::DriveEvent::Notice { text };
            self.publish(id, agent_threads::control::Change::Said { event });
        }
        let Some(command) = self.command(action).await else {
            self.threads.remove(&id);
            self.rights_now();
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
}
