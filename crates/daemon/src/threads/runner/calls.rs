//! The calls of the clients of this computer to the runner (TH12, TH13, TH15): each is checked
//! here, written to the log of the runner (THR-044), and answered on the socket.

use agent_threads::control::{Action, Call, Outcome, Refusal, Reply, RunState, ThreadRow};
use agent_threads::drive::{Effort, KeptFolder, Model, Program};
use uuid::Uuid;

use super::codex::Codex;
use super::log::{Kind, Line};
use super::socket::{self, CLIENT_BYTES, CLIENT_QUEUE, Caller, Client, Incoming};
use super::start::NewThread;
use super::{Runner, Waits};
use crate::threads::wire;
use answer::Answering;
use send::Sender;

mod answer;
mod attach;
mod commands;
mod files;
mod folders;
mod permissions;
mod queue;
use permissions::for_the_person;
mod reads;
mod refusal_lines;
mod send;
mod set;
pub(super) use commands::FolderSkills;
pub(super) use files::Walked;
use refusal_lines::Say;
pub(super) use refusal_lines::{RefusalLines, refusal_word};
pub(super) use send::SentPrompt;

/// What the runner can see of the source of a call: the local socket (THR-044).
const SOURCE: &str = "local socket";
/// The most clients that watch at one time.
const WATCHERS: usize = 16;

/// The Codex calls of a thread of Codex.
/// Its sandbox keeps the folder of the daemon read-only when the folder of the thread holds it
/// (THR-041).
pub(super) fn codex_of(
    row: &ThreadRow,
    effort: Option<Effort>,
    daemon: &std::path::Path,
) -> Option<Codex> {
    (row.program == Program::Codex).then(|| {
        let model = row.model.as_deref().and_then(|name| Model::new(name).ok());
        let mut codex = Codex::new(row.folder.clone(), row.mode, model, effort).keeping(
            crate::threads::notice::held_anima(&row.folder, daemon)
                .map(|anima| KeptFolder::new(anima, row.id)),
        );
        codex.change_speed(row.speed.clone());
        codex
    })
}

impl Runner {
    pub(super) async fn call(&mut self, incoming: Incoming) {
        let Incoming {
            call: Call { id, action },
            client,
            agent,
        } = incoming;
        let back = Caller {
            call: id,
            client: client.clone(),
        };
        let named = thread_of(&action);
        let answer = match self.read(&action) {
            Some(read) => read,
            None => match action {
                ref asked if agent && for_the_person(asked) => Outcome::Refused {
                    why: Refusal::FromAnAgent,
                },
                Action::Watch => {
                    self.watch(id, client);
                    return;
                }
                Action::Models => {
                    self.models_call(back, !agent);
                    return;
                }
                Action::Interrupt { thread } => self.interrupt(thread).await,
                queued @ (Action::Queue { .. }
                | Action::Requeue { .. }
                | Action::Unqueue { .. }) => self.queue_call(queued).await,
                set @ Action::Set { .. } => self.set(set).await,
                listed @ Action::Commands { .. } => self.commands_call(listed).await,
                Action::Compact { thread } => self.compact(thread).await,
                attach @ (Action::Attach { .. }
                | Action::AttachMore { .. }
                | Action::Unattach { .. }) => self.attach_call(attach),
                Action::ChangeListing { thread, change } => self.change_listing(thread, &change),
                project @ (Action::CreateProject { .. } | Action::CloneProject { .. }) => {
                    self.prepare_project(project, back);
                    return;
                }
                Action::ChangeProject { change } => self.change_project(&change),
                Action::Files {
                    thread,
                    query,
                    most,
                } => self.files(thread, &query, most),
                Action::Folder { folder } => self.folder_notice(&folder),
                Action::Folders { folder, hidden } => {
                    self.folders(folder, hidden, back);
                    return;
                }
                Action::RaiseLimit => self.raise_limit(),
                waits => match self.held_call(waits, back.clone(), !agent).await {
                    Ok(()) => return,
                    Err(why) => Outcome::Refused { why },
                },
            },
        };
        self.answer_call(&back, named, answer);
    }

    pub(super) fn answer_call(&mut self, back: &Caller, named: Option<Uuid>, answer: Outcome) {
        if let Outcome::Refused { why } = &answer {
            self.note(Kind::Refusal, named, refusal_word(why));
        }
        back.answer(answer);
    }

    /// The soft limits one step up, kept across a restart (THR-114).
    fn raise_limit(&mut self) -> Outcome {
        self.limits = self.limits.raised();
        self.save();
        Outcome::Limits {
            limits: self.limits,
        }
    }

    /// A call whose answer waits for the holder (a start, a prompt, an answer or a stop) or for
    /// a task of the Git engine.
    async fn held_call(
        &mut self,
        action: Action,
        back: Caller,
        person: bool,
    ) -> Result<(), Refusal> {
        match action {
            send @ Action::Send { .. } => {
                let sender = if person {
                    Sender::Person
                } else {
                    Sender::Agent
                };
                self.send(send, back, sender).await
            }
            Action::SendQueued { thread, queued } => self.send_queued(thread, queued, back).await,
            Action::Answer {
                thread,
                request,
                answer,
            } => {
                self.answer(Answering { thread, request }, &answer, back)
                    .await
            }
            Action::Stop { thread } => self.stop(thread, back).await,
            Action::Git {
                thread,
                repository,
                action,
            } => {
                self.git_call((thread, repository, action), !person, back);
                Ok(())
            }
            start => match NewThread::of(start) {
                Some(new) => self.start(new, back).await,
                None => Err(Refusal::BadValue),
            },
        }
    }

    /// A client that watches: the newest updates the runner keeps first, as many of each thread
    /// as the queue of the client holds, then each new one.
    fn watch(&mut self, id: u64, client: Client) {
        let threads = self.threads.len().max(1);
        let (share, bytes) = ((CLIENT_QUEUE - 64) / threads, (CLIENT_BYTES / 2) / threads);
        let mut told = socket::tell(
            &client,
            &Reply::Done {
                id,
                result: Outcome::Ok,
            },
        );
        for thread in self.threads.values() {
            for update in thread.kept_newest(share, bytes) {
                told = told && socket::tell(&client, &Reply::Update(Box::new((*update).clone())));
            }
        }
        // This watch alone gets the row and the waiting requests of each thread after the kept
        // updates, so that a request whose update is older than its share still has a card. The
        // other watches already have them (THR-030).
        for thread in self.threads.values_mut() {
            for change in thread.now_changes() {
                let update = thread.number(change);
                told = told && socket::tell(&client, &Reply::Update(Box::new(update)));
            }
        }
        if told {
            // The oldest watch gives way and is closed, so that the watches of this user stay
            // few and the client of that watch knows it (THR-045).
            if self.watchers.len() >= WATCHERS {
                self.watchers.remove(0).close();
            }
            self.watchers.push(client);
        }
    }

    /// Says once in the runner log that a program runs at background priority, because its
    /// lift failed (R-403).
    pub(super) fn say_priority(&mut self) {
        if crate::threads::priority::failure_to_say() {
            self.note(
                Kind::Start,
                None,
                "a program runs at background priority: the lift failed",
            );
        }
    }

    pub(super) fn note(&mut self, kind: Kind, thread: Option<Uuid>, what: &str) {
        let at = chrono::Utc::now();
        if kind == Kind::Refusal {
            let say = self.refusal_lines.admit(thread, at.timestamp() / 60);
            refusal_to_daemon_log(thread, what, say);
        }
        let line = Line {
            at,
            kind,
            thread,
            source: SOURCE,
            what,
        };
        if let Err(error) = self.log.write(&line) {
            tracing::warn!(%error, "the log of the runner was not written");
        }
    }

    async fn interrupt(&mut self, id: Uuid) -> Outcome {
        let Some(thread) = self.threads.get_mut(&id) else {
            return Outcome::Refused {
                why: Refusal::UnknownThread,
            };
        };
        if thread.row.state == RunState::Stopped {
            return Outcome::Refused {
                why: Refusal::NotRunning,
            };
        }
        let steps = thread.interrupt();
        if self.apply(id, steps).await.is_err() {
            return Outcome::Refused {
                why: Refusal::Holder,
            };
        }
        self.save();
        self.note(Kind::Stop, Some(id), "turn stopped");
        Outcome::Ok
    }

    async fn stop(&mut self, id: Uuid, back: Caller) -> Result<(), Refusal> {
        let thread = self.threads.get(&id).ok_or(Refusal::UnknownThread)?;
        if thread.row.state == RunState::Stopped {
            return Err(Refusal::NotRunning);
        }
        let command = self
            .command(wire::Action::Stop {
                thread: id.to_string(),
            })
            .await
            .ok_or(Refusal::Holder)?;
        self.note(Kind::Stop, Some(id), "program stopped");
        self.waits.insert(command, Waits::Stop { back });
        Ok(())
    }
}

/// The thread a call names, for the line of the log of a refusal (THR-044).
const fn thread_of(action: &Action) -> Option<Uuid> {
    match action {
        Action::Send { thread, .. }
        | Action::Answer { thread, .. }
        | Action::Interrupt { thread }
        | Action::Stop { thread }
        | Action::ChangeListing { thread, .. }
        | Action::Earlier { thread, .. }
        | Action::Files { thread, .. }
        | Action::Queue { thread, .. }
        | Action::Requeue { thread, .. }
        | Action::Unqueue { thread, .. }
        | Action::SendQueued { thread, .. }
        | Action::Set { thread, .. }
        | Action::Compact { thread }
        | Action::Attach { thread, .. }
        | Action::AttachMore { thread, .. }
        | Action::Unattach { thread, .. }
        | Action::Git { thread, .. } => Some(*thread),
        _ => None,
    }
}

/// The daemon's own log (MET-019) takes the refusals too, so that a person who reads only that
/// file sees them; the word is fixed, never text of the person or of the program (LOG-007).
/// At most [`refusal_lines::PER_THREAD`] lines of a thread in a minute; the last says so.
fn refusal_to_daemon_log(thread: Option<Uuid>, why: &str, say: Say) {
    let thread = thread.map_or_else(|| "none".to_owned(), |id| id.to_string());
    let more = match say {
        Say::Nothing => return,
        Say::Line => "",
        Say::LastLine => "; more refusals of this minute are only in runner.log",
    };
    tracing::info!(%thread, why, "the runner refused a call{more}");
}
