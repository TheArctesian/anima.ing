//! The skills of Codex in a folder before any thread runs there (TH117). Codex finds them: the
//! runner starts `codex app-server` in `/` and asks `skills/list` for the folder. The first call
//! for a folder answers `pending` and starts the ask; a later call gets the list. In memory only.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use agent_threads::drive::{CodexInput, CommandLists, DriveEvent, Program, codex_skills};
use serde_json::Value;

use super::super::super::login::asking::{self, Heard};

/// How long a list stays fresh; after it, a call asks again and gets the old list meanwhile.
const FRESH: Duration = Duration::from_secs(60);
/// The most folders whose lists are kept, and the most asks that run at one time.
const MOST_FOLDERS: usize = 16;
const MOST_ASKS: usize = 2;

/// The lists of the folders, shared with the asks that fill them.
#[derive(Clone, Debug, Default)]
pub struct FolderSkills(Arc<Mutex<BTreeMap<PathBuf, Entry>>>);

#[derive(Debug)]
struct Entry {
    at: Instant,
    lists: Option<CommandLists>,
    asking: bool,
}

/// What a call gets now, and whether it must start an ask.
pub struct Now {
    pub lists: Option<CommandLists>,
    pub ask: bool,
}

impl FolderSkills {
    /// The list of `folder` known now. When there is none or it is old, and few asks run, the
    /// folder is marked as asked: the caller starts the ask.
    pub fn now(&self, folder: &Path) -> Now {
        let mut folders = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let asking = folders.values().filter(|entry| entry.asking).count();
        let entry = folders.get(folder);
        let lists = entry.and_then(|entry| entry.lists.clone());
        let stale = entry.is_none_or(|entry| !entry.asking && entry.at.elapsed() > FRESH);
        let ask = stale && asking < MOST_ASKS;
        if ask {
            if folders.len() >= MOST_FOLDERS && !folders.contains_key(folder) {
                let oldest = folders
                    .iter()
                    .filter(|(_, entry)| !entry.asking)
                    .min_by_key(|(_, entry)| entry.at)
                    .map(|(folder, _)| folder.clone());
                oldest.map(|oldest| folders.remove(&oldest));
            }
            let lists = lists.clone();
            folders.insert(
                folder.to_owned(),
                Entry {
                    at: Instant::now(),
                    lists,
                    asking: true,
                },
            );
        }
        Now { lists, ask }
    }

    /// The end of an ask: its list, or the list before it when Codex gave none.
    fn asked(&self, folder: &Path, lists: Option<CommandLists>) {
        let mut folders = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(entry) = folders.get_mut(folder) {
            entry.asking = false;
            entry.at = Instant::now();
            if lists.is_some() {
                entry.lists = lists;
            }
        }
    }

    /// Asks Codex at `path` for the skills of `folder`, in the background.
    pub fn ask(&self, folder: PathBuf, path: PathBuf, env: Vec<(String, String)>) {
        let skills = self.clone();
        tokio::spawn(async move {
            let first = CodexInput::initialize(1);
            let wanted = folder.clone();
            let asked = asking::Asked {
                program: Program::Codex,
                path,
                env,
            };
            let lists = asking::ask(asked, first, move |value| heard(value, &wanted)).await;
            if let Err(fault) = lists {
                tracing::info!(
                    why = fault.words(),
                    "Codex gave no list of the skills of a folder"
                );
            }
            skills.asked(&folder, lists.ok());
        });
    }
}

fn heard(value: &Value, folder: &Path) -> Heard<CommandLists> {
    if asking::initialized(value) {
        return Heard::Write(vec![
            CodexInput::initialized(),
            CodexInput::skills_list(2, folder),
        ]);
    }
    if value["id"] != 2 {
        return Heard::Nothing;
    }
    match codex_skills(&value["result"]) {
        Some(DriveEvent::Commands { lists }) => Heard::Done(lists),
        // An answer that holds no list (an error) ends the ask with an empty list.
        _ => Heard::Done(CommandLists::default()),
    }
}
