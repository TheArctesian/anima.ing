//! The calls `commands` and `compact` (TH117): the commands and the skills of a program for the
//! `/` and `$` lists of the composer, and a compact of the context of a Codex thread. Only the
//! person makes them: the lists name files of the person.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use agent_threads::control::{Action, Outcome, Refusal};
use agent_threads::drive::{CommandLists, Program};
use uuid::Uuid;

use super::super::log::Kind;
use super::super::{Runner, login, models};

mod claude_files;
mod codex_folder;

pub use codex_folder::FolderSkills;

impl Runner {
    /// The lists of a thread, or of a program in a folder.
    pub(super) async fn commands_call(&mut self, action: Action) -> Outcome {
        let Action::Commands {
            thread,
            program,
            folder,
        } = action
        else {
            return refused(Refusal::BadValue);
        };
        match (thread, program, folder) {
            (Some(id), None, None) => self.thread_commands(id).await,
            (None, Some(program), Some(folder)) => self.folder_commands(program, folder).await,
            _ => refused(Refusal::BadValue),
        }
    }

    async fn thread_commands(&mut self, id: Uuid) -> Outcome {
        let Some(thread) = self.threads.get_mut(&id) else {
            return refused(Refusal::UnknownThread);
        };
        let (lists, again) = thread.commands(std::time::Instant::now());
        // A failed ask leaves the list pending: the next call asks again.
        let _ = self.apply(id, again).await;
        answer(lists)
    }

    async fn folder_commands(&self, program: Program, folder: PathBuf) -> Outcome {
        let daemon = self.dir.parent().unwrap_or(&self.dir).to_owned();
        match program {
            Program::ClaudeCode => self.claude_files(folder, daemon).await,
            Program::Grok => answer(Some(CommandLists::default())),
            Program::Codex => {
                let real = match super::super::start::chosen_folder(&folder, &daemon) {
                    Ok(real) => real,
                    Err(why) => return refused(why),
                };
                let found = login::find(program, &self.env, login::executable, login::trusted);
                let Some(path) = found else {
                    return refused(Refusal::ProgramNotFound);
                };
                let now = self.folder_skills.now(&real);
                if now.ask {
                    let env = models::environment(program, self.env.clone(), &path);
                    self.folder_skills.ask(real, path, env);
                }
                answer(now.lists)
            }
        }
    }

    /// The command files of Claude Code in `folder` and the home, read off the loop of the
    /// runner: a folder that does not answer in time (a hung mount) gives `pending`. One read
    /// runs at a time, so a read that never ends holds one thread, not one for each call.
    async fn claude_files(&self, folder: PathBuf, daemon: PathBuf) -> Outcome {
        if READING.swap(true, Ordering::AcqRel) {
            return answer(None);
        }
        let home = self.home();
        let read = tokio::task::spawn_blocking(move || {
            let _done = Reading;
            let real = super::super::start::chosen_folder(&folder, &daemon)?;
            Ok(claude_files::commands(&real, home.as_deref()))
        });
        match tokio::time::timeout(READ_TIME, read).await {
            Ok(Ok(Ok(commands))) => answer(Some(CommandLists {
                commands,
                skills: Vec::new(),
            })),
            Ok(Ok(Err(why))) => refused(why),
            Ok(Err(_)) | Err(_) => answer(None),
        }
    }

    /// The home folder of the login, whose `.claude` holds the commands of the person.
    fn home(&self) -> Option<PathBuf> {
        self.env
            .iter()
            .find(|(name, _)| name == "HOME")
            .map(|(_, home)| PathBuf::from(home))
            .filter(|home| home.is_absolute())
    }

    /// Codex compacts the context of the thread.
    pub(super) async fn compact(&mut self, id: Uuid) -> Outcome {
        let Some(thread) = self.threads.get_mut(&id) else {
            return refused(Refusal::UnknownThread);
        };
        match thread.compact() {
            Ok(steps) => {
                if self.apply(id, steps).await.is_err() {
                    return refused(Refusal::Holder);
                }
                self.note(Kind::Change, Some(id), "compact asked");
                Outcome::Ok
            }
            Err(why) => refused(why),
        }
    }
}

/// A read of command files runs.
static READING: AtomicBool = AtomicBool::new(false);

/// The end of a read, also by a panic.
struct Reading;

impl Drop for Reading {
    fn drop(&mut self) {
        READING.store(false, Ordering::Release);
    }
}

/// How long the runner waits for the command files of a folder.
const READ_TIME: std::time::Duration = std::time::Duration::from_secs(5);

const fn refused(why: Refusal) -> Outcome {
    Outcome::Refused { why }
}

fn answer(lists: Option<CommandLists>) -> Outcome {
    Outcome::Commands {
        pending: lists.is_none(),
        lists: lists.unwrap_or_default(),
    }
}
