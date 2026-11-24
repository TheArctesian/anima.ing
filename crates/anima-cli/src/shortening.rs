//! Shortened MCP answers (SHA-001 to SHA-018); the rules are in the crate `shortening`.

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

pub const FOLDER: &str = "shortening";

/// The part of an answer that the marked answer must save, in percent (SHA-007).
const LEAST_SAVING_PERCENT: usize = 15;

pub struct Shortening {
    store: Option<Store>,
    measure_dir: PathBuf,
}

impl Shortening {
    #[must_use]
    pub const fn off() -> Self {
        Self {
            store: None,
            measure_dir: PathBuf::new(),
        }
    }

    #[must_use]
    pub fn from_setting(config_dir: &Path, origin: &str, never: bool) -> Self {
        let on = !never && setting::read(&config_dir.join(FOLDER)) == setting::State::On;
        Self {
            store: on.then(|| Store::new(originals_dir(config_dir, origin))),
            measure_dir: config_dir.join(FOLDER),
        }
    }

    #[must_use]
    pub const fn store(&self) -> Option<&Store> {
        self.store.as_ref()
    }

    /// After each fault, or when shortening saves too little, `fields` as it is (SHA-006, SHA-007).
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

pub struct Worth {
    pub marked: Value,
    pub original: Original,
    /// The bytes of JSON of the whole answer and of the marked answer.
    pub before: usize,
    pub after: usize,
}

/// The marked answer fits whole, so no later limit cuts rows after the mark (SHA-004, SHA-007).
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

/// One folder per hub, so a handle never reads an answer of a different hub.
#[must_use]
pub fn originals_dir(config_dir: &Path, origin: &str) -> PathBuf {
    let hub = &store::handle_of(origin)[..16];
    config_dir.join(FOLDER).join(format!("{HUB}{hub}"))
}

#[cfg(test)]
mod tests;
