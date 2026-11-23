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
        let Some(short) = shortened(&Value::Object(fields.clone()), &Limits::default()) else {
            return fields;
        };
        let original = Original {
            tool: tool.to_owned(),
            lists: short.cuts().iter().map(|cut| cut.list.clone()).collect(),
            answer: Value::Object(fields),
        };
        let before = original.answer.to_string().len();
        let handle = store::handle_of(&original.answer.to_string());
        let marked = short.marked(&handle, original::TOOL);
        let after = marked.to_string().len();
        // The marked answer must fit whole: the last limit of a result (`fitted`) would cut
        // rows after the mark was written, and the mark would not be true (SHA-004). And the
        // saving is tested again with the real mark (SHA-007).
        let worth = after <= RESULT_CAP && after * 100 <= before * (100 - LEAST_SAVING_PERCENT);
        match marked {
            Value::Object(marked) if worth && store.put(&original, now).is_ok() => {
                measure::record(&self.measure_dir, tool, before, after);
                marked
            }
            _ => match original.answer {
                Value::Object(fields) => fields,
                _ => unreachable!("built from an object above"),
            },
        }
    }
}

/// The folder of the originals of one hub: an agent of a run for one hub cannot read, by a
/// handle, an answer of a different hub.
#[must_use]
pub fn originals_dir(config_dir: &Path, origin: &str) -> PathBuf {
    let hub = &store::handle_of(origin)[..16];
    config_dir.join(FOLDER).join(format!("hub-{hub}"))
}

#[cfg(test)]
mod tests;
