//! Shortened answers of the MCP server (SHA-001 to SHA-018): a long tool answer loses rows
//! that say the same, its original is kept in a private folder of this computer, and the
//! answer names the handle that gets it. The rules are in the crate `shortening`; this folder
//! holds the files, the setting, the measure, the commands and the tool.

pub mod commands;
pub mod measure;
pub mod original;
pub mod setting;
pub mod store;

use std::path::Path;
use std::time::SystemTime;

use serde_json::{Map, Value};
use shortening::{Limits, shortened};

use store::Store;

/// The folder of the originals, the setting and the measure, in the configuration folder.
pub const FOLDER: &str = "shortening";

/// The shortening of one run of the server: on with a store, or off.
pub struct Shortening {
    store: Option<Store>,
}

impl Shortening {
    /// Off: each answer stays as it is.
    #[must_use]
    pub const fn off() -> Self {
        Self { store: None }
    }

    /// As the setting in `config_dir` says, unless `never` (the flag of this run).
    #[must_use]
    pub fn from_setting(config_dir: &Path, never: bool) -> Self {
        let dir = config_dir.join(FOLDER);
        let on = !never && setting::read(&dir) == setting::State::On;
        Self {
            store: on.then(|| Store::new(dir)),
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
        let marked = shortened(&whole, &Limits::default()).and_then(|short| {
            let original = whole.to_string();
            let handle = store.put(tool, &whole, now).ok()?;
            let marked = short.marked(&handle, original::TOOL);
            measure::record(store.dir(), tool, original.len(), marked.to_string().len());
            Some(marked)
        });
        match marked.unwrap_or(whole) {
            Value::Object(fields) => fields,
            _ => unreachable!("an object stays an object"),
        }
    }
}

#[cfg(test)]
mod tests;
