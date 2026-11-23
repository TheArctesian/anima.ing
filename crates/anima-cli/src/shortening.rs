//! Shortened answers of the MCP server (SHA-001 to SHA-018): a long tool answer loses rows
//! that say the same, its original is kept in a private folder of this computer, and the
//! answer names the handle that gets it. The rules are in the crate `shortening`; this folder
//! holds the files, the setting, the measure, the commands and the tool.

pub mod commands;
pub mod measure;
pub mod original;
pub mod setting;
pub mod store;

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::{Map, Value};
use shortening::{Limits, shortened};

use crate::mcp::untrusted::RESULT_CAP;
use store::{Original, Store};

/// The folder of the originals, the setting and the measure, in the configuration folder.
pub const FOLDER: &str = "shortening";

/// The part of an answer that the marked answer must save, in percent (SHA-007).
const LEAST_SAVING_PERCENT: usize = 15;

/// The shortening of one run of the server: on with a store, or off.
pub struct Shortening {
    store: Option<Store>,
    measure_dir: PathBuf,
}

impl Shortening {
    /// Off: each answer stays as it is.
    #[must_use]
    pub const fn off() -> Self {
        Self {
            store: None,
            measure_dir: PathBuf::new(),
        }
    }

    /// As the setting in `config_dir` says, unless `never` (the flag of this run), with the
    /// originals of the hub `origin`.
    #[must_use]
    pub fn from_setting(config_dir: &Path, origin: &str, never: bool) -> Self {
        let on = !never && setting::read(&config_dir.join(FOLDER)) == setting::State::On;
        Self {
            store: on.then(|| Store::new(originals_dir(config_dir, origin))),
            measure_dir: config_dir.join(FOLDER),
        }
    }

    /// The store, when the shortening is on.
    #[must_use]
    pub const fn store(&self) -> Option<&Store> {
        self.store.as_ref()
    }

    /// The answer `fields` of `tool`, shortened and marked when that saves enough and its
    /// original is stored; in each other case, and after each fault, `fields` as it is
    /// (SHA-006, SHA-007).
    #[must_use]
    pub fn applied(
        &self,
        tool: &str,
        fields: Map<String, Value>,
        now: SystemTime,
    ) -> Map<String, Value> {
        let Some(store) = &self.store else {
            return fields;
        };
        let whole = Value::Object(fields);
        let given = worth_it(tool, &whole).and_then(|worth| {
            // The original is stored before the shortened answer is given (SHA-006).
            store.put(&worth.original, now).ok()?;
            measure::record(&self.measure_dir, tool, worth.before, worth.after);
            Some(worth.marked)
        });
        match given.unwrap_or(whole) {
            Value::Object(fields) => fields,
            _ => unreachable!("an object stays an object"),
        }
    }
}

/// A shortened answer that is worth giving, with what must be stored first.
pub struct Worth {
    /// The answer with its mark.
    pub marked: Value,
    /// The whole answer, with the lists that lost rows.
    pub original: Original,
    /// The bytes of JSON of the whole answer and of the marked answer.
    pub before: usize,
    pub after: usize,
}

/// `whole` shortened and marked; `None` when it stays as it is. The marked answer must fit
/// whole: the last limit of a result (`fitted`) would cut rows after the mark was written, and
/// the mark would not be true (SHA-004). And the saving is tested with the real mark (SHA-007).
#[must_use]
pub fn worth_it(tool: &str, whole: &Value) -> Option<Worth> {
    let short = shortened(whole, &Limits::default())?;
    let text = whole.to_string();
    let lists = short.cuts().iter().map(|cut| cut.list.clone()).collect();
    let marked = short.marked(&store::handle_of(&text), original::TOOL);
    let (before, after) = (text.len(), marked.to_string().len());
    let worth = after <= RESULT_CAP && after * 100 <= before * (100 - LEAST_SAVING_PERCENT);
    worth.then(|| Worth {
        marked,
        original: Original {
            tool: tool.to_owned(),
            answer: whole.clone(),
            lists,
        },
        before,
        after,
    })
}

/// Removes the originals that are too old in the folder of each hub, so that the originals of
/// a hub that is not used any more do not stay. A fault leaves them for the next start.
pub fn remove_old_of_each_hub(config_dir: &Path, now: SystemTime) {
    let Ok(folders) = std::fs::read_dir(config_dir.join(FOLDER)) else {
        return;
    };
    for folder in folders.flatten() {
        let hub = folder
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with(HUB));
        if hub && folder.file_type().is_ok_and(|kind| kind.is_dir()) {
            // The next start tries again (SHA-006: this never stops an answer).
            let _left = Store::new(folder.path()).remove_old(now);
        }
    }
}

const HUB: &str = "hub-";

/// The folder of the originals of one hub: an agent of a run for one hub cannot read, by a
/// handle, an answer of a different hub.
#[must_use]
pub fn originals_dir(config_dir: &Path, origin: &str) -> PathBuf {
    let hub = &store::handle_of(origin)[..16];
    config_dir.join(FOLDER).join(format!("{HUB}{hub}"))
}

#[cfg(test)]
mod tests;
