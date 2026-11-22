//! The calls that only read what the runner holds: the threads, the listings, the projects, the
//! accounts, the models and the earlier updates of a thread (TH106).

use std::path::Path;

use agent_threads::control::{Action, Outcome, Refusal};

use super::super::Runner;
use super::super::socket::CLIENT_BYTES;

impl Runner {
    /// The answer to a call that only reads, or `None` for each other call.
    pub(super) fn read(&self, action: &Action) -> Option<Outcome> {
        Some(match action {
            Action::List => Outcome::Threads {
                threads: self
                    .threads
                    .values()
                    .map(|thread| thread.row.clone())
                    .collect(),
                holder: self.holder_words(),
            },
            Action::Listings => self.listings(),
            Action::Projects => self.projects(),
            Action::Accounts => Outcome::Accounts {
                accounts: self.said.accounts.rows(),
            },
            Action::Models => Outcome::Models {
                models: self.said.models.rows(),
            },
            Action::Earlier {
                thread,
                before,
                most,
            } => self.threads.get(thread).map_or(
                Outcome::Refused {
                    why: Refusal::UnknownThread,
                },
                |thread| Outcome::Earlier {
                    updates: thread
                        .kept_before(*before, *most, CLIENT_BYTES / 2)
                        .cloned()
                        .collect(),
                },
            ),
            _ => return None,
        })
    }

    /// What the composer says of a folder that the person chose for a new thread, or the refusal of it.
    pub(super) fn folder_notice(&self, folder: &Path) -> Outcome {
        let daemon = self.dir.parent().unwrap_or(&self.dir);
        match super::super::start::chosen_folder(folder, daemon) {
            Ok(real) => Outcome::Folder {
                notice: super::super::start::notice_of(&real, daemon),
            },
            Err(why) => Outcome::Refused { why },
        }
    }
}
